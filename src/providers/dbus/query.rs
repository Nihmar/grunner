//! D-Bus query execution for search providers

use crate::core::global_state::get_tokio_runtime;
use futures::stream::{FuturesUnordered, StreamExt};
use gtk4::gdk::Display;
use gtk4::prelude::DisplayExt;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use super::icons::parse_icon_variant;
use super::types::{SearchProvider, SearchResult};

fn get_runtime() -> &'static tokio::runtime::Runtime {
    get_tokio_runtime()
}

async fn get_or_init_conn() -> zbus::Result<Connection> {
    static DBUS_CONN: OnceLock<Connection> = OnceLock::new();
    if let Some(c) = DBUS_CONN.get() {
        return Ok(c.clone());
    }
    let conn = Connection::session().await?;
    Ok(DBUS_CONN.get_or_init(|| conn).clone())
}

pub fn run_search_streaming(
    providers: &[SearchProvider],
    query: &str,
    max_per_provider: usize,
    tx: std::sync::mpsc::Sender<Vec<SearchResult>>,
) {
    let terms: Vec<String> = query.split_whitespace().map(String::from).collect();
    if terms.is_empty() {
        return;
    }
    get_runtime().block_on(query_all_streaming(providers, &terms, max_per_provider, tx));
}

async fn query_all_streaming(
    providers: &[SearchProvider],
    terms: &[String],
    max_per_provider: usize,
    tx: std::sync::mpsc::Sender<Vec<SearchResult>>,
) {
    debug!(
        "Starting search across {} providers with terms: {:?}",
        providers.len(),
        terms
    );
    for provider in providers {
        debug!(
            "  - {} (bus: {}, path: {})",
            provider.desktop_id, provider.bus_name, provider.object_path
        );
    }

    let conn = match get_or_init_conn().await {
        Ok(c) => c,
        Err(e) => {
            error!("Cannot connect to D-Bus session bus: {e}");
            return;
        }
    };

    let terms_str: Vec<&str> = terms.iter().map(String::as_str).collect();

    let mut futs: FuturesUnordered<_> = providers
        .iter()
        .map(|provider| {
            let conn = conn.clone();
            let terms_str = terms_str.clone();
            let bus_name = provider.bus_name.clone();
            async move {
                let result = query_one(&conn, provider, &terms_str, max_per_provider).await;
                (bus_name, result)
            }
        })
        .collect();

    while let Some((bus_name, outcome)) = futs.next().await {
        match outcome {
            Ok(results) if !results.is_empty() => {
                debug!("Provider {} returned {} results", bus_name, results.len());
                if tx.send(results).is_err() {
                    debug!("Search provider channel closed, stopping processing");
                    break;
                }
            }
            Err(e) => {
                error!("Search provider {bus_name} error: {e}");
            }
            _ => {
                debug!("Provider {bus_name} returned empty result set");
            }
        }
    }
}

async fn query_one(
    conn: &Connection,
    provider: &SearchProvider,
    terms: &[&str],
    max_results: usize,
) -> zbus::Result<Vec<SearchResult>> {
    use tokio::time::timeout;

    debug!(
        "Querying search provider: {} with terms: {:?}",
        provider.bus_name, terms
    );

    let proxy = zbus::Proxy::new(
        conn,
        provider.bus_name.as_str(),
        provider.object_path.as_str(),
        "org.gnome.Shell.SearchProvider2",
    )
    .await?;

    let timeout_dur = Duration::from_secs(3);

    let ids: Vec<String> = timeout(timeout_dur, proxy.call("GetInitialResultSet", &(terms,)))
        .await
        .map_err(|_| {
            zbus::Error::Failure("D-Bus call to GetInitialResultSet timed out".into())
        })??;

    debug!(
        "Provider {} returned {} result IDs: {:?}",
        provider.bus_name,
        ids.len(),
        ids
    );

    if ids.is_empty() {
        debug!("Provider {} returned empty result set", provider.bus_name);
        return Ok(vec![]);
    }

    let ids_capped: Vec<&str> = ids.iter().take(max_results).map(String::as_str).collect();

    let metas: Vec<HashMap<String, OwnedValue>> =
        timeout(timeout_dur, proxy.call("GetResultMetas", &(ids_capped,)))
            .await
            .map_err(|_| zbus::Error::Failure("D-Bus call to GetResultMetas timed out".into()))??;

    debug!(
        "Provider {} returned {} result metas",
        provider.bus_name,
        metas.len()
    );

    let results: Vec<SearchResult> = metas
        .into_iter()
        .filter_map(|meta| build_result(meta, provider, &provider.app_icon))
        .collect();

    debug!(
        "Provider {} successfully returned {} search results",
        provider.bus_name,
        results.len()
    );

    Ok(results)
}

fn build_result(
    mut meta: HashMap<String, OwnedValue>,
    provider: &SearchProvider,
    app_icon: &str,
) -> Option<SearchResult> {
    let id = take_str(&mut meta, "id")?;
    let name = take_str(&mut meta, "name").unwrap_or_else(|| id.clone());
    let description = take_str(&mut meta, "description").unwrap_or_default();

    if let Some(val) = meta.get("clipboardText")
        && let Ok(text) = String::try_from(val.clone())
    {
        if let Some(display) = Display::default() {
            let clipboard = display.clipboard();
            clipboard.set_text(&text);
            info!("Copied '{text}' to clipboard from search provider metadata");
        } else {
            warn!("No default GDK Display available — cannot copy to clipboard");
        }
    }

    let icon = meta.get("icon").and_then(parse_icon_variant);

    Some(SearchResult {
        id,
        name,
        description,
        icon,
        app_icon: app_icon.to_string(),
        bus_name: provider.bus_name.clone(),
        object_path: provider.object_path.clone(),
    })
}

fn take_str(meta: &mut HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    use zbus::zvariant::Value;

    let val = meta.remove(key)?;

    match &*val {
        Value::Str(s) => Some(s.as_str().to_string()),
        Value::Value(inner) => {
            if let Value::Str(s) = &**inner {
                Some(s.as_str().to_string())
            } else {
                None
            }
        }
        _ => String::try_from(val).ok(),
    }
}

pub fn activate_result(
    bus_name: &str,
    object_path: &str,
    result_id: &str,
    terms: &[String],
    timestamp: u32,
) {
    let bus_name = bus_name.to_string();
    let object_path = object_path.to_string();
    let result_id = result_id.to_string();
    let terms = terms.to_vec();
    debug!("Activating search result: {result_id} from provider {bus_name}");

    get_runtime().block_on(async move {
        let Ok(conn) = get_or_init_conn().await else {
            error!("Cannot connect to D-Bus session bus for result activation");
            return;
        };
        let Ok(proxy) = zbus::Proxy::new(
            &conn,
            bus_name.as_str(),
            object_path.as_str(),
            "org.gnome.Shell.SearchProvider2",
        )
        .await
        else {
            error!("Failed to create D-Bus proxy for provider {bus_name}");
            return;
        };

        let terms_str: Vec<&str> = terms.iter().map(String::as_str).collect();
        if let Err(e) = proxy
            .call::<_, _, ()>(
                "ActivateResult",
                &(result_id.as_str(), &terms_str, timestamp),
            )
            .await
        {
            error!("Failed to activate result {result_id}: {e}");
        } else {
            info!("Successfully activated search result: {result_id}");
        }
    });
}
