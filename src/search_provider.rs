//! GNOME Shell search provider integration for Grunner
//!
//! This module provides integration with GNOME Shell search providers via D-Bus,
//! allowing Grunner to query external applications for search results and display
//! them in the launcher interface.
//!
//! Key features:
//! - Discovery of installed search providers from .ini files
//! - Asynchronous D-Bus communication with providers
//! - Streaming result updates with real-time UI feedback
//! - Icon parsing for both themed icons and file-based icons
//! - Blacklist support for excluding unwanted providers
//!
//! The module uses a combination of async/await (via Tokio) and D-Bus (via zbus)
//! to communicate with search providers while keeping the UI responsive.

use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

// ---------------------------------------------------------------------------
// Global runtime and connection management
// ---------------------------------------------------------------------------

/// Global Tokio runtime for async D-Bus operations
///
/// Search providers use asynchronous D-Bus calls which require a Tokio runtime.
/// This static runtime is initialized once and shared across all provider queries.
static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Global D-Bus session connection
///
/// D-Bus connections are expensive to create, so we reuse a single connection
/// for all search provider queries throughout the application's lifetime.
static DBUS_CONN: OnceLock<Connection> = OnceLock::new();

/// Get or initialize the shared Tokio runtime
///
/// Creates a multi-threaded runtime with a single worker thread optimized
/// for I/O operations and timing, suitable for D-Bus communication.
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

/// Get or initialize the shared D-Bus session connection
///
/// Returns a clone of the global connection, establishing it first if needed.
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

/// Represents a GNOME Shell search provider
///
/// This struct contains the D-Bus addressing information and metadata
/// needed to communicate with a search provider. Providers are discovered
/// from .ini files in standard search provider directories.
#[derive(Debug, Clone)]
pub struct SearchProvider {
    /// D-Bus bus name (e.g., "org.gnome.Nautilus")
    pub bus_name: String,
    /// D-Bus object path (e.g., "/org/gnome/Nautilus/SearchProvider")
    pub object_path: String,
    /// Icon name resolved from the provider's .desktop file at discovery time
    ///
    /// Used as a fallback icon for results when individual results don't
    /// provide their own icons.
    pub app_icon: String,
    /// Desktop ID (e.g., "org.gnome.Nautilus.desktop") used for blacklisting
    ///
    /// This matches the .desktop filename and allows users to exclude
    /// specific providers via configuration.
    pub desktop_id: String,
}

/// Icon data carried by a search result
///
/// GNOME Shell search providers can send icons in two formats:
/// 1. Themed icon names that reference the current GTK icon theme
/// 2. File paths to image files (used for thumbnails, custom icons, etc.)
#[derive(Debug, Clone)]
pub enum IconData {
    /// Named icon from the current GTK icon theme (e.g., "text-x-generic")
    Themed(String),
    /// Absolute filesystem path to an image file
    File(String),
}

/// Individual search result from a provider
///
/// This struct contains all the information needed to display and activate
/// a search result, including metadata, icons, and D-Bus addressing for
/// activation when the user selects the result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Unique identifier for this result within the provider
    pub id: String,
    /// Display name for the result
    pub name: String,
    /// Descriptive text (optional, may be empty)
    pub description: String,
    /// Icon specific to this result, if the provider sent one
    pub icon: Option<IconData>,
    /// Icon of the provider application itself (from its .desktop file)
    ///
    /// Used as a fallback when individual results don't have icons.
    pub app_icon: String,
    /// D-Bus bus name of the provider (needed for result activation)
    pub bus_name: String,
    /// D-Bus object path of the provider (needed for result activation)
    pub object_path: String,
}

// ---------------------------------------------------------------------------
// Provider discovery
// ---------------------------------------------------------------------------

/// Discover all available GNOME Shell search providers
///
/// Scans standard directories for .ini files describing search providers,
/// parses them, and filters out any providers in the blacklist.
///
/// # Arguments
/// * `blacklist` - List of desktop IDs to exclude from discovery
///
/// # Returns
/// Vector of discovered `SearchProvider` instances, ready for querying.
pub fn discover_providers(blacklist: &[String]) -> Vec<SearchProvider> {
    let home = std::env::var("HOME").unwrap_or_default();
    // Standard directories where GNOME Shell search providers are installed
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
            // Only process .ini files (GNOME Shell search provider definitions)
            if path.extension().map(|e| e == "ini").unwrap_or(false) {
                if let Some(p) = parse_ini(&path) {
                    // Skip if this provider's desktop_id is in the blacklist
                    if blacklist.iter().any(|b| b == &p.desktop_id) {
                        continue;
                    }
                    providers.push(p);
                }
            }
        }
    }
    providers
}

/// Parse a single .ini file into a SearchProvider
///
/// GNOME Shell search provider .ini files contain D-Bus addressing
/// information and metadata in a simple key=value format.
///
/// # Arguments
/// * `path` - Path to the .ini file to parse
///
/// # Returns
/// `Some(SearchProvider)` if the file is valid and version 2,
/// `None` otherwise.
fn parse_ini(path: &std::path::Path) -> Option<SearchProvider> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut bus_name = None;
    let mut object_path = None;
    let mut desktop_id = None;
    let mut version: Option<u32> = None;

    // Parse simple key=value format
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

    // Only support version 2 of the search provider API
    if version != Some(2) {
        return None;
    }

    let desktop_id = desktop_id?;

    Some(SearchProvider {
        bus_name: bus_name?,
        object_path: object_path?,
        app_icon: resolve_app_icon(&desktop_id),
        desktop_id,
    })
}

/// Resolve the icon name from a desktop ID
///
/// Looks up the .desktop file for a given desktop ID and extracts
/// the Icon= field to determine what icon to use for this provider.
///
/// # Arguments
/// * `desktop_id` - Desktop ID (with or without .desktop extension)
///
/// # Returns
/// Icon name string, or empty string if not found.
pub fn resolve_app_icon(desktop_id: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();

    // Ensure we have the .desktop extension
    let filename = if desktop_id.ends_with(".desktop") {
        desktop_id.to_string()
    } else {
        format!("{}.desktop", desktop_id)
    };

    // Search in standard desktop entry directories
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
// Icon parsing
// ---------------------------------------------------------------------------

/// Parse icon data from a D-Bus variant value
///
/// GNOME Shell search providers can send icons in several complex formats:
/// - Simple string (themed icon name)
/// - Structure with type and payload (themed-icon or file-icon)
/// - Nested variants and dictionaries
///
/// This function handles all these cases to extract usable icon information.
///
/// # Arguments
/// * `val` - D-Bus variant value containing icon data
///
/// # Returns
/// `Some(IconData)` if a valid icon was found, `None` otherwise.
fn parse_icon_variant(val: &OwnedValue) -> Option<IconData> {
    use zbus::zvariant::Value;

    // Inner recursive function to walk the variant tree
    fn inner(v: &Value<'_>) -> Option<IconData> {
        match v {
            // Unwrap nested variants first
            Value::Value(inner_v) => inner(inner_v),

            // The main case: (type_str, dict_or_array)
            // This is the standard format: ("themed-icon", {"names": [...]})
            // or ("file-icon", {"file": "path"})
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

/// Extract themed icon name from a GThemedIcon payload
///
/// The payload is the second field of (sa{sv}) structure, which is a dict `a{sv}`.
/// We look for a key "names" whose value is an array of strings (icon names).
///
/// # Arguments
/// * `val` - D-Bus value containing themed icon data
///
/// # Returns
/// `Some(IconData::Themed)` with the first icon name found, or `None`.
fn extract_themed(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    // Helper to extract first non-empty string from an array
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

    // Walk the D-Bus value looking for "names" key
    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Dict(d) => {
                // First, try to find the "names" key explicitly
                for (k, val) in d.iter() {
                    if let (Value::Str(key), v2) = (k, val) {
                        if key.as_str() == "names" {
                            if let Some(name) = first_name_from_array(v2) {
                                return Some(name);
                            }
                        }
                    }
                }

                // Fallback: search any array in the dict
                for (_, val) in d.iter() {
                    if let Some(name) = first_name_from_array(val) {
                        return Some(name);
                    }
                }
                None
            }

            Value::Array(_) => first_name_from_array(v),
            _ => None,
        }
    }

    walk(val).map(IconData::Themed)
}

/// Extract file path from a GFileIcon payload
///
/// The payload is the second field of (sa{sv}) structure. We look for
/// a key "file" or a string value containing a file path.
///
/// # Arguments
/// * `val` - D-Bus value containing file icon data
///
/// # Returns
/// `Some(IconData::File)` with the file path, or `None`.
fn extract_file(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    // Walk the D-Bus value looking for file paths
    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Str(s) => {
                let s = s.as_str();
                if !s.is_empty() {
                    // Strip file:// URI prefix if present
                    let path = s.strip_prefix("file://").unwrap_or(s);
                    Some(path.to_string())
                } else {
                    None
                }
            }
            Value::Dict(d) => {
                // Look for "file" key explicitly
                for (k, val) in d.iter() {
                    if let Value::Str(key) = k {
                        if key.as_str() == "file" {
                            if let Some(p) = walk(val) {
                                return Some(p);
                            }
                        }
                    }
                }

                // Fallback: search any value in the dict
                d.iter().find_map(|(_, v)| walk(v))
            }
            _ => None,
        }
    }

    walk(val).map(IconData::File)
}

// ---------------------------------------------------------------------------
// Search execution
// ---------------------------------------------------------------------------

/// Execute a search query across all providers with streaming results
///
/// This is the main entry point for search provider queries. It:
/// 1. Splits the query into search terms
/// 2. Runs async queries across all providers in parallel
/// 3. Sends results back through a channel as they arrive
///
/// # Arguments
/// * `providers` - List of providers to query
/// * `query` - Search query string
/// * `max_per_provider` - Maximum results per provider
/// * `tx` - Channel sender for streaming results
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

    // Block on the async runtime to execute the search
    get_runtime().block_on(query_all_streaming(providers, &terms, max_per_provider, tx));
}

/// Async implementation of streaming search across all providers
///
/// Queries all providers concurrently using FuturesUnordered, which allows
/// results to be processed as soon as they arrive from any provider.
async fn query_all_streaming(
    providers: &[SearchProvider],
    terms: &[String],
    max_per_provider: usize,
    tx: std::sync::mpsc::Sender<Vec<SearchResult>>,
) {
    // Get or create D-Bus connection
    let conn = match get_or_init_conn().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[search] cannot connect to session bus: {}", e);
            return;
        }
    };

    // Convert terms to &str for D-Bus call
    let terms_str: Vec<&str> = terms.iter().map(String::as_str).collect();

    // Create futures for all providers and collect them into an unordered set
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

    // Process results as they complete
    while let Some((bus_name, outcome)) = futs.next().await {
        match outcome {
            Ok(results) if !results.is_empty() => {
                // Send batch of results back to main thread
                if tx.send(results).is_err() {
                    break; // Channel closed, stop processing
                }
            }
            Err(e) => eprintln!("[search] provider {} error: {}", bus_name, e),
            _ => {} // Empty results or other non-error cases
        }
    }
}

/// Query a single search provider
///
/// Performs the actual D-Bus calls to a provider:
/// 1. GetInitialResultSet - gets result IDs for the search terms
/// 2. GetResultMetas - gets metadata for the result IDs
///
/// Each call has a timeout to prevent hanging on unresponsive providers.
async fn query_one(
    conn: &Connection,
    provider: &SearchProvider,
    terms: &[&str],
    max_results: usize,
) -> zbus::Result<Vec<SearchResult>> {
    use tokio::time::timeout;

    // Create D-Bus proxy for the search provider
    let proxy = zbus::Proxy::new(
        conn,
        provider.bus_name.as_str(),
        provider.object_path.as_str(),
        "org.gnome.Shell.SearchProvider2",
    )
    .await?;

    // Timeout for D-Bus calls (3 seconds)
    let timeout_dur = Duration::from_secs(3);

    // Step 1: Get result IDs for the search terms
    let ids: Vec<String> = timeout(timeout_dur, proxy.call("GetInitialResultSet", &(terms,)))
        .await
        .map_err(|_| {
            zbus::Error::Failure("D-Bus call to GetInitialResultSet timed out".into())
        })??;

    if ids.is_empty() {
        return Ok(vec![]);
    }

    // Cap the number of results to requested maximum
    let ids_capped: Vec<&str> = ids.iter().take(max_results).map(String::as_str).collect();

    // Step 2: Get metadata for the capped result IDs
    let metas: Vec<HashMap<String, OwnedValue>> =
        timeout(timeout_dur, proxy.call("GetResultMetas", &(ids_capped,)))
            .await
            .map_err(|_| zbus::Error::Failure("D-Bus call to GetResultMetas timed out".into()))??;

    // Convert D-Bus metadata to SearchResult structs
    let results = metas
        .into_iter()
        .filter_map(|meta| build_result(meta, provider, &provider.app_icon))
        .collect();

    Ok(results)
}

/// Build a SearchResult from D-Bus metadata
///
/// Extracts fields from the D-Bus dictionary and handles missing or
/// malformed data with reasonable defaults.
fn build_result(
    mut meta: HashMap<String, OwnedValue>,
    provider: &SearchProvider,
    app_icon: &str,
) -> Option<SearchResult> {
    let id = take_str(&mut meta, "id")?;
    let name = take_str(&mut meta, "name").unwrap_or_else(|| id.clone());
    let description = take_str(&mut meta, "description").unwrap_or_default();

    // Parse icon if present
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

/// Extract a string value from a D-Bus dictionary
///
/// Removes the key from the dictionary and attempts to convert the value
/// to a String. Returns None if the key doesn't exist or conversion fails.
fn take_str(meta: &mut HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let val = meta.remove(key)?;
    String::try_from(val).ok()
}

// ---------------------------------------------------------------------------
// Result activation
// ---------------------------------------------------------------------------

/// Activate a search result (open it in the provider application)
///
/// When a user selects a search result, this function notifies the
/// provider via D-Bus so it can perform the appropriate action
/// (open file, launch application, etc.).
///
/// # Arguments
/// * `bus_name` - D-Bus bus name of the provider
/// * `object_path` - D-Bus object path of the provider
/// * `result_id` - ID of the result to activate
/// * `terms` - Original search terms (for context)
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

        // Generate a timestamp for the activation (required by D-Bus API)
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
