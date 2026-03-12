//! Workspace window bar for Grunner.
//!
//! Renders a horizontal strip of buttons — one per open window on the current
//! GNOME workspace — placed between the search entry and the results list.
//!
//! ## Architecture
//!
//! Data is fetched asynchronously via two GNOME D-Bus interfaces:
//!
//! - **`org.gnome.Shell.Introspect`** (`GetWindows`) — the full window list with
//!   per-window properties (`title`, `workspace-index`, `wm-class`, …).
//! - **`org.gnome.Mutter.WorkspaceManager`** (`CurrentWorkspace` property) —
//!   the index of the active workspace so we can filter to only that workspace.
//!
//! Window activation is attempted via **`org.gnome.Shell.Eval`** (JavaScript
//! executed inside GNOME Shell).
//!
//! > **GNOME ≥ 43 note:** `Shell.Eval` requires the shell to be started with
//! > `--unsafe-mode`.  Without it the D-Bus call returns an error and the window
//! > button will still hide Grunner, but the target window may not come to the
//! > foreground.  A future improvement could drive activation via a dedicated
//! > GNOME Shell extension instead.
//!
//! ## Refresh strategy
//!
//! The bar subscribes to its own `map` signal so it re-queries D-Bus every time
//! the Grunner launcher window becomes visible.  No background polling is done
//! while the launcher is hidden.

use glib::clone;
use gtk4::{
    Box as GtkBox, Button, Image, Label, Orientation, PolicyType, ScrolledWindow, gdk, prelude::*,
};
use libadwaita::ApplicationWindow;
use std::collections::HashMap;
use zbus::{Connection, proxy, zvariant::OwnedValue};

// ─── D-Bus proxy definitions ──────────────────────────────────────────────────

/// Proxy for `org.gnome.Shell.Introspect` — window enumeration.
#[proxy(
    interface = "org.gnome.Shell.Introspect",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Introspect"
)]
trait ShellIntrospect {
    /// Returns every open window as `window_id → { property_name → value }`.
    ///
    /// Relevant property keys and D-Bus types:
    /// - `"title"` — `s`
    /// - `"wm-class"` — `s`
    /// - `"app-id"` — `s`  (desktop-file stem, empty for many X11 apps)
    /// - `"sandboxed-app-id"` — `s`  (Flatpak reverse-domain ID when present)
    /// - `"workspace-index"` — `u`  (0-based workspace number)
    /// - `"is-on-all-workspaces"` — `b`
    fn get_windows(&self) -> zbus::Result<HashMap<u64, HashMap<String, OwnedValue>>>;
}

/// Proxy for `org.gnome.Mutter.WorkspaceManager` — active workspace index.
#[proxy(
    interface = "org.gnome.Mutter.WorkspaceManager",
    default_service = "org.gnome.Mutter",
    default_path = "/org/gnome/Mutter/WorkspaceManager"
)]
trait MutterWorkspaceManager {
    /// 0-based index of the currently visible workspace.
    #[zbus(property)]
    fn current_workspace(&self) -> zbus::Result<u32>;
}

/// Proxy for `org.gnome.Shell` — JavaScript evaluation (used for window activation).
#[proxy(
    interface = "org.gnome.Shell",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell"
)]
trait GnomeShell {
    /// Evaluate a JS expression inside the running GNOME Shell process.
    ///
    /// Returns `(success: bool, result_string: String)`.
    fn eval(&self, script: &str) -> zbus::Result<(bool, String)>;
}

// ─── Internal data model ──────────────────────────────────────────────────────

/// Lightweight representation of a single open window.
#[derive(Debug, Clone)]
struct WindowInfo {
    /// Internal Mutter window ID (used in the `Shell.Eval` activation script).
    id: u64,
    /// Human-readable window title as set by the client application.
    title: String,
    /// Best-effort GTK icon theme name derived from the window's app identity.
    ///
    /// Resolution order: Flatpak `sandboxed-app-id` → generic `app-id` →
    /// lowercase `wm-class` → `"application-x-executable"` fallback.
    icon_name: String,
}

// ─── zvariant helpers — unwrap one level of Value::Value if present ──────────

fn unwrap_value(v: &OwnedValue) -> &zbus::zvariant::Value<'_> {
    use zbus::zvariant::Value;
    match &**v {
        Value::Value(inner) => inner.as_ref(),
        other => other,
    }
}

fn val_str(v: &OwnedValue) -> Option<String> {
    use zbus::zvariant::Value;
    match unwrap_value(v) {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

fn val_u32(v: &OwnedValue) -> Option<u32> {
    use zbus::zvariant::Value;
    match unwrap_value(v) {
        Value::U32(n) => Some(*n),
        Value::I32(n) => Some(*n as u32), // defensive: some builds expose i32
        _ => None,
    }
}

fn val_bool(v: &OwnedValue) -> Option<bool> {
    use zbus::zvariant::Value;
    match unwrap_value(v) {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

// ─── Async D-Bus queries ──────────────────────────────────────────────────────

/// Query GNOME Shell for all windows on the active workspace.
///
/// Returns `None` on any D-Bus error (e.g. not running under GNOME Shell,
/// or the service is not yet available at startup).  The bar will simply
/// remain hidden in that case rather than showing an error.
async fn fetch_workspace_windows() -> Option<Vec<WindowInfo>> {
    let conn = Connection::session()
        .await
        .map_err(|e| log::warn!("[workspace_bar] D-Bus session connect failed: {e}"))
        .ok()?;

    let introspect = ShellIntrospectProxy::new(&conn)
        .await
        .map_err(|e| log::warn!("[workspace_bar] ShellIntrospect proxy failed: {e}"))
        .ok()?;

    let ws_mgr = MutterWorkspaceManagerProxy::new(&conn)
        .await
        .map_err(|e| log::warn!("[workspace_bar] WorkspaceManager proxy failed: {e}"))
        .ok()?;

    let raw = introspect
        .get_windows()
        .await
        .map_err(|e| {
            log::warn!("[workspace_bar] GetWindows() failed: {e}");
            if format!("{e}").contains("AccessDenied") {
                log::warn!("[workspace_bar] Permission denied. GNOME Shell may need to be started with --unsafe-mode, or a GNOME Shell extension may be required.");
            }
        })
        .ok()?;

    let current_ws = ws_mgr
        .current_workspace()
        .await
        .map_err(|e| log::warn!("[workspace_bar] CurrentWorkspace failed: {e}"))
        .ok()?;

    log::debug!(
        "[workspace_bar] GetWindows returned {} entries, current_ws={}",
        raw.len(),
        current_ws
    );

    let mut windows: Vec<WindowInfo> = raw
        .into_iter()
        .filter_map(|(id, props): (u64, HashMap<String, OwnedValue>)| {
            let title = props.get("title").and_then(val_str)?;
            if title.is_empty() {
                return None;
            }

            let on_all = props
                .get("is-on-all-workspaces")
                .and_then(val_bool)
                .unwrap_or(false);

            // If the property is missing entirely, include the window rather
            // than silently discarding it — better to over-show than under-show.
            let ws_idx = props
                .get("workspace-index")
                .and_then(val_u32)
                .unwrap_or(current_ws); // absent → assume current workspace

            log::debug!(
                "[workspace_bar]  id={id} ws={ws_idx} on_all={on_all} title={title:?} \
                 props_keys={:?}",
                props.keys().collect::<Vec<_>>()
            );

            if !on_all && ws_idx != current_ws {
                return None;
            }

            let icon_name = props
                .get("sandboxed-app-id")
                .and_then(val_str)
                .filter(|s: &String| !s.is_empty())
                .or_else(|| {
                    props
                        .get("app-id")
                        .and_then(val_str)
                        .filter(|s: &String| !s.is_empty())
                })
                .or_else(|| {
                    props
                        .get("wm-class")
                        .and_then(val_str)
                        .map(|s: String| s.to_lowercase())
                })
                .unwrap_or_else(|| "application-x-executable".to_string());

            Some(WindowInfo {
                id,
                title,
                icon_name,
            })
        })
        .collect();

    log::debug!("[workspace_bar] {} window(s) passed filter", windows.len());

    windows.sort_by(|a, b| a.title.cmp(&b.title));
    Some(windows)
}

/// Ask GNOME Shell to bring a window to the foreground using its Mutter ID.
///
/// Executes a short JavaScript snippet via `org.gnome.Shell.Eval`.
/// Silently does nothing if the call fails (see module-level note on GNOME ≥ 43).
async fn activate_window(id: u64) {
    let Ok(conn) = Connection::session().await else {
        return;
    };
    let Ok(shell) = GnomeShellProxy::new(&conn).await else {
        return;
    };

    // Find the MetaWindow by its internal ID and call activate(timestamp=0).
    let script = format!(
        "global.get_window_actors()\
         .map(a=>a.get_meta_window())\
         .find(w=>w.get_id()=={id})\
         ?.activate(0);"
    );
    // Ignore the result — failure is non-fatal (window may still be raised
    // via the OS compositor even when Eval is restricted).
    let _ = shell.eval(&script).await;
}

// ─── Widget helpers ───────────────────────────────────────────────────────────

/// Maximum title length (in Unicode scalar values) shown inside a button.
const MAX_TITLE_CHARS: usize = 22;

/// Return `s` truncated to `max` characters, with a trailing `…` if cut.
fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Resolve the best matching icon name available in `theme`, trying a few
/// common variations of `preferred` before falling back to a generic icon.
fn resolve_icon(preferred: &str, theme: &gtk4::IconTheme) -> String {
    // Direct match (covers Flatpak IDs like "org.gnome.Nautilus")
    if theme.has_icon(preferred) {
        return preferred.to_owned();
    }
    // Lowercase variant (covers wm-class values like "Nautilus" → "nautilus")
    let lower = preferred.to_lowercase();
    if theme.has_icon(&lower) {
        return lower;
    }
    // Last segment only, lowercase (covers "org.gnome.Nautilus" → "nautilus")
    if let Some(last) = preferred.rsplit('.').next() {
        let last_lower = last.to_lowercase();
        if theme.has_icon(&last_lower) {
            return last_lower;
        }
    }
    "application-x-executable".to_owned()
}

// ─── Populate ─────────────────────────────────────────────────────────────────

/// Clear and refill `buttons_box` with one button per entry in `windows`.
///
/// The outer `scroll` container is shown when there are windows to display and
/// hidden when the list is empty.  This is the only place where the bar's
/// visibility is mutated.
fn populate(
    buttons_box: &GtkBox,
    scroll: &ScrolledWindow,
    windows: Vec<WindowInfo>,
    icon_theme: &gtk4::IconTheme,
    app_window: &ApplicationWindow,
) {
    log::debug!(
        "[workspace_bar] populate called with {} window(s), scroll visible={}",
        windows.len(),
        scroll.is_visible()
    );

    // Clear existing buttons (fast — GObjects are reference-counted).
    while let Some(child) = buttons_box.first_child() {
        buttons_box.remove(&child);
    }

    if windows.is_empty() {
        log::debug!("[workspace_bar] no windows, hiding scroll");
        scroll.set_visible(false);
        return;
    }

    for info in windows {
        log::debug!(
            "[workspace_bar] creating button id={} title={:?} icon={:?}",
            info.id,
            info.title,
            info.icon_name
        );

        let btn = Button::new();
        btn.add_css_class("workspace-window-btn");
        btn.set_tooltip_text(Some(&info.title));

        // Horizontal inner layout: [icon] [label]
        let inner = GtkBox::new(Orientation::Horizontal, 4);
        inner.add_css_class("workspace-window-btn-inner");

        let resolved_icon = resolve_icon(&info.icon_name, icon_theme);
        log::debug!(
            "[workspace_bar] resolved icon {:?} → {:?}",
            info.icon_name,
            resolved_icon
        );
        let icon = Image::from_icon_name(&resolved_icon);
        icon.add_css_class("workspace-window-icon");
        inner.append(&icon);

        let label = Label::new(Some(&truncate(&info.title, MAX_TITLE_CHARS)));
        label.add_css_class("workspace-window-label");
        inner.append(&label);

        btn.set_child(Some(&inner));

        // On click: ask the compositor to focus the window, then hide Grunner.
        let win_id = info.id;
        btn.connect_clicked(clone!(
            #[weak]
            app_window,
            move |_| {
                glib::spawn_future_local(async move {
                    activate_window(win_id).await;
                });
                app_window.hide();
            }
        ));

        buttons_box.append(&btn);
    }

    log::debug!("[workspace_bar] populate done, showing scroll");
    scroll.set_visible(true);
}

// ─── Public constructor ───────────────────────────────────────────────────────

/// Build the workspace window bar and return it as a `ScrolledWindow`.
///
/// The returned widget:
/// - Is **invisible** until the first refresh completes (no flicker on cold start).
/// - Stays **hidden** when the current workspace has no open windows.
/// - **Auto-refreshes** every time it is mapped (i.e. every time the Grunner
///   launcher window is made visible) so it always reflects current state.
///
/// # Placement
/// Append the returned widget to the root layout container **after** the search
/// entry row and **before** the results `ScrolledWindow`:
/// ```rust
/// root.append(&entry_box);
/// root.append(&workspace_bar);   // ← here
/// root.append(&scrolled);        // existing results list
/// ```
pub fn build_workspace_bar(window: &ApplicationWindow) -> ScrolledWindow {
    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Never)
        .min_content_height(1)
        .build();
    scroll.add_css_class("workspace-bar");
    scroll.set_visible(false);

    let buttons_box = GtkBox::new(Orientation::Horizontal, 6);
    buttons_box.add_css_class("workspace-bar-buttons");
    scroll.set_child(Some(&buttons_box));

    // Use connect_map instead of connect_show — for top-level ApplicationWindows,
    // connect_map fires reliably when the compositor maps the window (every time
    // the launcher is shown).  connect_show may not propagate correctly through
    // GTK4-rs signal wrappers for decorated=false top-level windows.
    window.connect_map(clone!(
        #[weak]
        scroll,
        #[weak]
        buttons_box,
        #[weak]
        window,
        move |_| {
            log::debug!("[workspace_bar] connect_map fired, launching fetch thread");

            let (tx, rx) = std::sync::mpsc::channel::<Vec<WindowInfo>>();

            // Spawn background thread with its own tokio rt to drive zbus.
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("[workspace_bar] failed to build tokio runtime");
                let windows = rt.block_on(fetch_workspace_windows()).unwrap_or_default();
                log::debug!(
                    "[workspace_bar] background thread sending {} window(s)",
                    windows.len()
                );
                let _ = tx.send(windows);
            });

            // Poll from the GTK main thread, identical to how ui.rs polls app loading.
            glib::idle_add_local_once(clone!(
                #[weak]
                scroll,
                #[weak]
                buttons_box,
                #[weak]
                window,
                move || {
                    poll_windows(rx, scroll, buttons_box, window);
                }
            ));
        }
    ));

    scroll
}

fn poll_windows(
    rx: std::sync::mpsc::Receiver<Vec<WindowInfo>>,
    scroll: ScrolledWindow,
    buttons_box: GtkBox,
    window: ApplicationWindow,
) {
    match rx.try_recv() {
        Ok(windows) => {
            log::debug!(
                "[workspace_bar] poll_windows received {} window(s)",
                windows.len()
            );
            let Some(display) = gdk::Display::default() else {
                return;
            };
            let icon_theme = gtk4::IconTheme::for_display(&display);
            populate(&buttons_box, &scroll, windows, &icon_theme, &window);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // Thread not done yet — reschedule.
            glib::idle_add_local_once(move || poll_windows(rx, scroll, buttons_box, window));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            log::warn!("[workspace_bar] background thread disconnected without sending");
        }
    }
}
