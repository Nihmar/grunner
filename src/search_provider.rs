use futures::stream::{FuturesUnordered, StreamExt};
/// GNOME Shell Search Provider 2 integration.
use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

// ---------------------------------------------------------------------------
// Cached tokio runtime & D-Bus session connection
// ---------------------------------------------------------------------------

/// A single multi-thread tokio runtime (1 worker) that lives for the process
/// lifetime.  Keeping it alive means the zbus `Connection` (which spawns
/// internal tasks on the runtime) never loses its executor.
static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Session-bus connection, created once and reused for every search.
static DBUS_CONN: OnceLock<Connection> = OnceLock::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_io()
            .enable_time()
            .build()
            .expect("[search] failed to build tokio runtime")
    })
}

async fn get_or_init_conn() -> zbus::Result<Connection> {
    if let Some(c) = DBUS_CONN.get() {
        return Ok(c.clone());
    }
    let conn = Connection::session().await?;
    Ok(DBUS_CONN.get_or_init(|| conn).clone())
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SearchProvider {
    pub bus_name: String,
    pub object_path: String,
    /// Icon name resolved once from the .desktop file at discovery time.
    pub app_icon: String,
}

/// Icon carried by a search result — two possible representations.
#[derive(Debug, Clone)]
pub enum IconData {
    /// Named icon from the current GTK icon theme (e.g. "text-x-generic").
    Themed(String),
    /// Absolute filesystem path to an image file (thumbnails, etc.).
    File(String),
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Icon specific to this result, if the provider sent one.
    pub icon: Option<IconData>,
    /// Icon of the provider application itself (read from its .desktop file).
    /// Used as fallback when `icon` is None.
    pub app_icon: String,
    pub bus_name: String,
    pub object_path: String,
}

// ---------------------------------------------------------------------------
// Provider discovery
// ---------------------------------------------------------------------------

fn is_blacklisted(desktop_id: &str) -> bool {
    let blacklist = ["epiphany"];
    let desktop_id_lower = desktop_id.to_lowercase();
    blacklist.iter().any(|&b| desktop_id_lower.contains(b))
}

pub fn discover_providers() -> Vec<SearchProvider> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/gnome-shell/search-providers"),
        PathBuf::from(format!(
            "{}/.local/share/gnome-shell/search-providers",
            home
        )),
    ];

    let mut providers = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "ini").unwrap_or(false) {
                if let Some(p) = parse_ini(&path) {
                    providers.push(p);
                }
            }
        }
    }
    providers
}

fn parse_ini(path: &std::path::Path) -> Option<SearchProvider> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut bus_name = None;
    let mut object_path = None;
    let mut desktop_id = None;
    let mut version: Option<u32> = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("BusName=") {
            bus_name = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("ObjectPath=") {
            object_path = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("DesktopId=") {
            desktop_id = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("Version=") {
            version = v.parse().ok();
        }
    }

    if version != Some(2) {
        return None;
    }

    let desktop_id = desktop_id?; // now we require DesktopId
    if is_blacklisted(&desktop_id) {
        return None;
    }

    Some(SearchProvider {
        bus_name: bus_name?,
        object_path: object_path?,
        app_icon: resolve_app_icon(&desktop_id),
    })
}

// ---------------------------------------------------------------------------
// .desktop icon resolution
// ---------------------------------------------------------------------------

/// Read the `Icon=` field from a .desktop file identified by its desktop-id.
/// Returns an empty string if not found.
pub fn resolve_app_icon(desktop_id: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    // Strip .desktop suffix if already present, then re-add it.
    let filename = if desktop_id.ends_with(".desktop") {
        desktop_id.to_string()
    } else {
        format!("{}.desktop", desktop_id)
    };

    let search_dirs = [
        format!("/usr/share/applications/{}", filename),
        format!("{}/.local/share/applications/{}", home, filename),
        format!("/usr/local/share/applications/{}", filename),
    ];

    for path in &search_dirs {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some(icon) = line.trim().strip_prefix("Icon=") {
                    return icon.trim().to_string();
                }
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// GVariant icon parsing
// ---------------------------------------------------------------------------

/// Parse a GIcon serialized as a GVariant (as produced by g_icon_serialize).
///
/// The format on the wire is `(sa{sv})`:
///   - First field:  type string — "themed-icon" | "file-icon" | "bytes-icon"
///   - Second field: dict with type-specific keys
///
/// GThemedIcon  → key "names"  → as  (array of icon-theme names)
/// GFileIcon    → key "file"   → s   (absolute path or file:// URI)
fn parse_icon_variant(val: &OwnedValue) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn inner(v: &Value<'_>) -> Option<IconData> {
        match v {
            // Unwrap nested variants first
            Value::Value(inner_v) => inner(inner_v),

            // The main case: (type_str, dict_or_array)
            Value::Structure(s) => {
                let fields = s.fields();
                if fields.len() >= 2 {
                    if let Value::Str(type_name) = &fields[0] {
                        match type_name.as_str() {
                            "themed-icon" => {
                                return extract_themed(&fields[1]);
                            }
                            "file-icon" => {
                                return extract_file(&fields[1]);
                            }
                            _ => {}
                        }
                    }
                }
                // Unknown structure — walk fields looking for something usable
                fields.iter().find_map(inner)
            }

            // Plain string — treat as a themed icon name directly
            Value::Str(s) => {
                let s = s.as_str();
                if !s.is_empty() && !s.contains(' ') {
                    Some(IconData::Themed(s.to_string()))
                } else {
                    None
                }
            }

            _ => None,
        }
    }

    inner(val.deref())
}

/// Extract the first icon name from a GThemedIcon payload.
/// The payload is the second field of (sa{sv}), which is a dict `a{sv}`.
/// We look for a key "names" whose value is an array of strings.
fn extract_themed(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn first_name_from_array(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Array(a) => a.iter().find_map(|item| match item {
                Value::Str(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
                Value::Value(inner) => first_name_from_array(inner),
                _ => None,
            }),
            Value::Value(inner) => first_name_from_array(inner),
            Value::Str(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
            _ => None,
        }
    }

    // Walk the dict looking for "names"
    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Dict(d) => {
                // Try the "names" key first
                for (k, val) in d.iter() {
                    if let (Value::Str(key), v2) = (k, val) {
                        if key.as_str() == "names" {
                            if let Some(name) = first_name_from_array(v2) {
                                return Some(name);
                            }
                        }
                    }
                }
                // Fallback: any string array in the dict
                for (_, val) in d.iter() {
                    if let Some(name) = first_name_from_array(val) {
                        return Some(name);
                    }
                }
                None
            }
            // Sometimes the payload is just the array directly
            Value::Array(_) => first_name_from_array(v),
            _ => None,
        }
    }

    walk(val).map(IconData::Themed)
}

/// Extract the file path from a GFileIcon payload.
/// Looks for a "file" key whose value is a string path or file:// URI.
fn extract_file(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Str(s) => {
                let s = s.as_str();
                if !s.is_empty() {
                    // Strip file:// URI scheme if present
                    let path = s.strip_prefix("file://").unwrap_or(s);
                    Some(path.to_string())
                } else {
                    None
                }
            }
            Value::Dict(d) => {
                for (k, val) in d.iter() {
                    if let Value::Str(key) = k {
                        if key.as_str() == "file" {
                            if let Some(p) = walk(val) {
                                return Some(p);
                            }
                        }
                    }
                }
                // Any string in the dict
                d.iter().find_map(|(_, v)| walk(v))
            }
            _ => None,
        }
    }

    walk(val).map(IconData::File)
}

// ---------------------------------------------------------------------------
// Querying
// ---------------------------------------------------------------------------

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
    let conn = match get_or_init_conn().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[search] cannot connect to session bus: {}", e);
            return;
        }
    };

    let terms_str: Vec<&str> = terms.iter().map(String::as_str).collect();

    // Query all providers concurrently; stream results as each finishes.
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
                if tx.send(results).is_err() {
                    break; // receiver dropped — search cancelled
                }
            }
            Err(e) => eprintln!("[search] provider {} error: {}", bus_name, e),
            _ => {}
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

    let proxy = zbus::Proxy::new(
        conn,
        provider.bus_name.as_str(),
        provider.object_path.as_str(),
        "org.gnome.Shell.SearchProvider2",
    )
    .await?;

    // Timeout after 3 seconds for each D-Bus call
    let timeout_dur = Duration::from_secs(3);

    let ids: Vec<String> = timeout(timeout_dur, proxy.call("GetInitialResultSet", &(terms,)))
        .await
        .map_err(|_| {
            zbus::Error::Failure("D-Bus call to GetInitialResultSet timed out".into())
        })??;

    if ids.is_empty() {
        return Ok(vec![]);
    }

    let ids_capped: Vec<&str> = ids.iter().take(max_results).map(String::as_str).collect();

    let metas: Vec<HashMap<String, OwnedValue>> =
        timeout(timeout_dur, proxy.call("GetResultMetas", &(ids_capped,)))
            .await
            .map_err(|_| zbus::Error::Failure("D-Bus call to GetResultMetas timed out".into()))??;

    let results = metas
        .into_iter()
        .filter_map(|meta| build_result(meta, provider, &provider.app_icon))
        .collect();

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

    // Try to parse the result-specific icon; fall through to None if absent.
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
    let val = meta.remove(key)?;
    String::try_from(val).ok()
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

pub fn activate_result(bus_name: &str, object_path: &str, result_id: &str, terms: &[String]) {
    let bus_name = bus_name.to_string();
    let object_path = object_path.to_string();
    let result_id = result_id.to_string();
    let terms = terms.to_vec();

    get_runtime().block_on(async move {
        let Ok(conn) = get_or_init_conn().await else {
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
            return;
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        let terms_str: Vec<&str> = terms.iter().map(String::as_str).collect();
        if let Err(e) = proxy
            .call::<_, _, ()>(
                "ActivateResult",
                &(result_id.as_str(), &terms_str, timestamp),
            )
            .await
        {
            eprintln!("[search] ActivateResult error: {}", e);
        }
    });
}
