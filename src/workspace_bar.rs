//! Workspace window bar for Grunner.
//!
//! Renders a horizontal strip of buttons — one per open window on the current
//! GNOME workspace — placed between the search entry and the results list.
//!
//! ## Architecture
//!
//! Data is fetched via the **`window-calls`** GNOME Shell extension, which
//! exposes a D-Bus interface for window management on Wayland. Without this
//! extension, the D-Bus approach requires GNOME Shell's `--unsafe-mode` which
//! is unavailable in GNOME 43+.
//!
//! The extension provides:
//! - **`org.gnome.Shell.Extensions.Windows.List`** — returns JSON array of all
//!   windows with properties (`id`, `wm_class`, `workspace`, `in_current_workspace`, etc.)
//! - **`org.gnome.Shell.Extensions.Windows.GetTitle`** — returns window title string
//! - **`org.gnome.Shell.Extensions.Windows.Activate`** — activates a window by ID
//!
//! ## Installation
//!
//! Install the extension from https://extensions.gnome.org/extension/4724/window-calls/
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
use serde::Deserialize;
use zbus::{Connection, proxy};

// ─── D-Bus proxy definitions ──────────────────────────────────────────────────

/// Proxy for `org.gnome.Shell.Extensions.Windows` — window enumeration via
/// the window-calls GNOME Shell extension.
#[proxy(
    interface = "org.gnome.Shell.Extensions.Windows",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/Windows"
)]
trait WindowCalls {
    /// Returns JSON array of all windows with their properties.
    ///
    /// Each entry contains: `id`, `wm_class`, `wm_class_instance`, `pid`,
    /// `workspace`, `in_current_workspace`, `frame_type`, `window_type`, etc.
    fn list(&self) -> zbus::Result<String>;

    /// Returns the title of the window with the given ID.
    fn get_title(&self, win_id: u64) -> zbus::Result<String>;

    /// Activates (focuses) the window with the given ID.
    fn activate(&self, win_id: u64) -> zbus::Result<()>;
}

// ─── Internal data model ──────────────────────────────────────────────────────

/// Lightweight representation of a single open window.
#[derive(Debug, Clone)]
struct WindowInfo {
    /// Window ID used by Mutter (used for activation).
    id: u64,
    /// Human-readable window title as set by the client application.
    title: String,
    /// Best-effort GTK icon theme name derived from the window's app identity.
    ///
    /// Resolution order: lowercase `wm_class_instance` → lowercase `wm_class` →
    /// `"application-x-executable"` fallback.
    icon_name: String,
}

/// Raw window entry from the extension's List method.
#[derive(Debug, Deserialize)]
struct RawWindowEntry {
    id: u64,
    #[serde(rename = "wm_class")]
    wm_class: Option<String>,
    #[serde(rename = "wm_class_instance")]
    wm_class_instance: Option<String>,
    #[serde(rename = "in_current_workspace")]
    in_current_workspace: bool,
    pid: u64,
}

// ─── Async D-Bus queries ──────────────────────────────────────────────────────

/// Query the window-calls extension for all windows on the active workspace.
///
/// Returns `None` on any D-Bus error (e.g. extension not installed).
/// The bar will simply remain hidden in that case.
async fn fetch_workspace_windows() -> Option<Vec<WindowInfo>> {
    let our_pid = std::process::id();

    let conn = Connection::session()
        .await
        .map_err(|e| log::warn!("[workspace_bar] D-Bus session connect failed: {e}"))
        .ok()?;

    let windows = WindowCallsProxy::new(&conn)
        .await
        .map_err(|e| {
            log::warn!(
                "[workspace_bar] WindowCalls proxy failed: {e}. Is the window-calls \
                 extension installed? (https://extensions.gnome.org/extension/4724/window-calls/)"
            );
        })
        .ok()?;

    let json = windows
        .list()
        .await
        .map_err(|e| log::warn!("[workspace_bar] WindowCalls.List failed: {e}"))
        .ok()?;

    let raw_windows: Vec<RawWindowEntry> = serde_json::from_str(&json)
        .map_err(|e| log::warn!("[workspace_bar] Failed to parse window list JSON: {e}"))
        .ok()?;

    log::debug!(
        "[workspace_bar] List returned {} entries, our_pid={}",
        raw_windows.len(),
        our_pid
    );

    let mut result = Vec::new();

    for raw in raw_windows {
        if !raw.in_current_workspace {
            continue;
        }

        if raw.pid == u64::from(our_pid) {
            log::debug!("[workspace_bar] skipping grunner (pid={})", our_pid);
            continue;
        }

        let title = windows
            .get_title(raw.id)
            .await
            .map_err(|e| {
                log::debug!("[workspace_bar] GetTitle({}) failed: {e}", raw.id);
            })
            .ok()
            .filter(|t| !t.is_empty())
            .or_else(|| raw.wm_class.clone())
            .or_else(|| raw.wm_class_instance.clone())
            .unwrap_or_else(|| "Untitled".to_string());

        let icon_name = raw
            .wm_class_instance
            .as_ref()
            .or(raw.wm_class.as_ref())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "application-x-executable".to_string());

        log::debug!(
            "[workspace_bar]  id={} pid={} ws=current title={:?} icon={:?}",
            raw.id,
            raw.pid,
            title,
            icon_name
        );

        result.push(WindowInfo {
            id: raw.id,
            title,
            icon_name,
        });
    }

    log::debug!(
        "[workspace_bar] {} window(s) passed filter",
        result.len()
    );

    result.sort_by(|a, b| a.title.cmp(&b.title));
    Some(result)
}

/// Ask the window-calls extension to bring a window to the foreground.
async fn activate_window(id: u64) {
    let Ok(conn) = Connection::session().await else {
        return;
    };
    let Ok(windows) = WindowCallsProxy::new(&conn).await else {
        return;
    };

    let result = windows.activate(id).await;
    if let Err(e) = result {
        log::warn!("[workspace_bar] Activate({}) failed: {}", id, e);
    }
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
    if theme.has_icon(preferred) {
        return preferred.to_owned();
    }
    let lower = preferred.to_lowercase();
    if theme.has_icon(&lower) {
        return lower;
    }
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

    while let Some(child) = buttons_box.first_child() {
        buttons_box.remove(&child);
    }

    if windows.is_empty() {
        log::debug!("[workspace_bar] no windows, showing empty bar");
        scroll.set_visible(true);
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
/// - Shows an empty bar when there are no open windows.
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

    window.connect_map(clone!(
        #[weak]
        scroll,
        #[weak]
        buttons_box,
        #[weak]
        window,
        move |_| {
            log::debug!("[workspace_bar] connect_map fired, launching fetch thread");

            let (tx, rx) = std::sync::mpsc::channel::<Option<Vec<WindowInfo>>>();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("[workspace_bar] failed to build tokio runtime");
                let windows = rt.block_on(fetch_workspace_windows());
                log::debug!(
                    "[workspace_bar] background thread result: {:?}",
                    windows.as_ref().map(|w| w.len())
                );
                let _ = tx.send(windows);
            });

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
    rx: std::sync::mpsc::Receiver<Option<Vec<WindowInfo>>>,
    scroll: ScrolledWindow,
    buttons_box: GtkBox,
    window: ApplicationWindow,
) {
    match rx.try_recv() {
        Ok(Some(windows)) => {
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
        Ok(None) => {
            log::debug!("[workspace_bar] extension not available, hiding bar");
            scroll.set_visible(false);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            glib::idle_add_local_once(move || poll_windows(rx, scroll, buttons_box, window));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            log::warn!("[workspace_bar] background thread disconnected without sending");
        }
    }
}
