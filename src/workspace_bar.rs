//! Workspace window bar for Grunner.
//!
//! Renders a horizontal strip of buttons — one per open window on the current
//! GNOME workspace — placed between the search entry and the results list.
//!
//! Requires the **window-calls** GNOME Shell extension:
//! <https://extensions.gnome.org/extension/4724/window-calls/>
//!
//! The bar auto-refreshes every time the Grunner launcher window becomes visible.

use crate::global_state::get_home_dir;
use glib::clone;
use gtk4::{
    Box as GtkBox, Button, EventControllerScroll, EventControllerScrollFlags, Image, Label,
    Orientation, PolicyType, PropagationPhase, ScrolledWindow, gdk, prelude::*,
};
use libadwaita::ApplicationWindow;
use serde::Deserialize;
use std::sync::OnceLock;
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

    /// Activates (focuses) the window with the given ID.
    fn activate(&self, win_id: u32) -> zbus::Result<()>;
}

// ─── Global runtime management ─────────────────────────────────────────────────

/// Global Tokio runtime for async workspace operations
///
/// This runtime is used for D-Bus communication with the window-calls extension.
/// It's shared across all workspace bar refresh operations to avoid the overhead
/// of creating a new runtime for each window map event.
static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or initialize the shared Tokio runtime
///
/// Creates a current-thread runtime optimized for I/O operations,
/// suitable for D-Bus communication with the window-calls extension.
fn get_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("[workspace_bar] failed to build tokio runtime")
    })
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
    title: Option<String>,
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
                 extension installed? (<https://extensions.gnome.org/extension/4724/window-calls/>)"
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

        let wm_class = raw.wm_class.as_deref().unwrap_or("");
        let wm_class_instance = raw.wm_class_instance.as_deref().unwrap_or("");

        if wm_class == "org.nihmar.grunner" || wm_class_instance == "org.nihmar.grunner" {
            continue;
        }

        // Try to get app name and icon from desktop file
        let (title, icon_from_desktop) = resolve_from_desktop(wm_class)
            .or_else(|| resolve_from_desktop(wm_class_instance))
            .map(|(n, i)| (n, Some(i)))
            .unwrap_or_else(|| (String::new(), None));

        let title = if !title.is_empty() {
            title
        } else {
            raw.title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| raw.wm_class.clone())
                .or_else(|| raw.wm_class_instance.clone())
                .unwrap_or_else(|| "Untitled".to_string())
        };

        let icon_name = icon_from_desktop
            .filter(|i| !i.is_empty())
            .or_else(|| {
                if !wm_class_instance.is_empty() {
                    Some(wm_class_instance.to_lowercase())
                } else if !wm_class.is_empty() {
                    Some(wm_class.to_lowercase())
                } else {
                    None
                }
            })
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

    log::debug!("[workspace_bar] {} window(s) passed filter", result.len());

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

    let result = windows.activate(id as u32).await;
    if let Err(e) = result {
        log::warn!("[workspace_bar] Activate({}) failed: {}", id, e);
    }
}

// ─── Widget helpers ───────────────────────────────────────────────────────────

/// Resolve app name and icon from desktop file using `wm_class`
fn resolve_from_desktop(wm_class: &str) -> Option<(String, String)> {
    let home = get_home_dir();

    let filename = format!("{}.desktop", wm_class);
    let search_dirs = [
        format!("/usr/share/applications/{}", filename),
        format!("{}/.local/share/applications/{}", home, filename),
        format!("/usr/local/share/applications/{}", filename),
        format!("/var/lib/flatpak/exports/share/applications/{}", filename),
        format!(
            "{}/.local/share/flatpak/exports/share/applications/{}",
            home, filename
        ),
    ];

    for path in &search_dirs {
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut name: Option<String> = None;
            let mut icon: Option<String> = None;
            for line in content.lines() {
                // Take the first Name= entry (usually the main app name)
                // Desktop files can have multiple entries like "New Window" for actions
                if name.is_none()
                    && line.trim().starts_with("Name=")
                    && let Some(n) = line.trim().strip_prefix("Name=")
                {
                    name = Some(n.trim().to_string());
                }
                if let Some(i) = line.trim().strip_prefix("Icon=") {
                    icon = Some(i.trim().to_string());
                }
            }
            if let Some(n) = name {
                let i = icon.unwrap_or_default();
                return Some((n, i));
            }
        }
    }
    None
}

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

    // Try with common prefix replacements
    let replacements = [
        ("org.gnome.", "gnome-"),
        ("org.freedesktop.", ""),
        ("com.", ""),
        ("net.", ""),
    ];
    for (prefix, replacement) in &replacements {
        if let Some(stripped) = preferred.strip_prefix(prefix) {
            let candidate = format!("{}{}", replacement, stripped);
            if theme.has_icon(&candidate) {
                return candidate;
            }
        }
    }

    // Try last segment only (e.g., "org.gnome.Nautilus" -> "nautilus")
    if let Some(last) = preferred.rsplit('.').next()
        && theme.has_icon(last)
    {
        return last.to_owned();
    }

    // Try with "gnome-" prefix
    if let Some(last) = preferred.rsplit('.').next() {
        let candidate = format!("gnome-{last}");
        if theme.has_icon(&candidate) {
            return candidate;
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
        log::debug!("[workspace_bar] no windows, hiding bar");
        scroll.set_visible(false);
        return;
    }

    let window_count = windows.len();

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

    if window_count > 6 {
        scroll.add_css_class("tall");
        buttons_box.set_margin_bottom(12);
    } else {
        scroll.remove_css_class("tall");
        buttons_box.set_margin_bottom(0);
    }
}

// ─── Public constructor ───────────────────────────────────────────────────────

/// Build the workspace window bar.
///
/// The widget is invisible until populated, hidden when no windows exist,
/// and expands taller when more than 6 windows require a scrollbar.
#[must_use]
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

    // Add event controller scroll to ensure scroll wheel events are handled
    // Use BOTH_AXES to handle both horizontal and vertical scroll events
    let scroll_controller = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
    scroll_controller.set_propagation_phase(PropagationPhase::Capture);

    // Connect to the scroll signal to handle scroll events
    let scroll_clone = scroll.clone();
    scroll_controller.connect_scroll(move |_, dx, dy| {
        log::debug!("[workspace_bar] scroll event: dx={dx}, dy={dy}");
        // Get the current adjustment for horizontal scrolling
        // For horizontal scrolling, we use dx (horizontal delta)
        // For vertical scroll wheel, we translate dy to horizontal scrolling
        let adjustment = scroll_clone.hadjustment();
        let delta = if dx == 0.0 { dy } else { dx };
        let new_value = adjustment.value() + delta * adjustment.step_increment();
        adjustment.set_value(new_value.clamp(0.0, adjustment.upper() - adjustment.page_size()));
        glib::Propagation::Stop
    });

    scroll.add_controller(scroll_controller);

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
                let rt = get_runtime();
                let windows = rt.block_on(fetch_workspace_windows());
                log::debug!(
                    "[workspace_bar] background thread result: {:?}",
                    windows.as_ref().map(std::vec::Vec::len)
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
