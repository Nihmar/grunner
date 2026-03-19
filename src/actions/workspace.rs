//! Workspace window operations via D-Bus
//!
//! Provides D-Bus communication with the window-calls GNOME Shell extension
//! to enumerate, activate, and close windows on the current workspace.

use crate::utils::desktop::resolve_desktop_info;
use log::{debug, warn};
use serde::Deserialize;
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.gnome.Shell.Extensions.Windows",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/Windows"
)]
trait WindowCalls {
    fn list(&self) -> zbus::Result<String>;
    fn activate(&self, win_id: u32) -> zbus::Result<()>;
    fn close(&self, win_id: u32) -> zbus::Result<()>;
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub icon_name: String,
}

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

pub async fn fetch_workspace_windows() -> Option<Vec<WindowInfo>> {
    let our_pid = std::process::id();

    let conn = Connection::session()
        .await
        .map_err(|e| warn!("[workspace] D-Bus session connect failed: {e}"))
        .ok()?;

    let windows = WindowCallsProxy::new(&conn)
        .await
        .map_err(|e| {
            warn!(
                "[workspace] WindowCalls proxy failed: {e}. Is the window-calls extension installed?"
            );
        })
        .ok()?;

    let json = windows
        .list()
        .await
        .map_err(|e| warn!("[workspace] WindowCalls.List failed: {e}"))
        .ok()?;

    let raw_windows: Vec<RawWindowEntry> = serde_json::from_str(&json)
        .map_err(|e| warn!("[workspace] Failed to parse window list JSON: {e}"))
        .ok()?;

    debug!(
        "[workspace] List returned {} entries, our_pid={}",
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

        let (title, icon_from_desktop) = resolve_desktop_info(wm_class)
            .or_else(|| resolve_desktop_info(wm_class_instance))
            .map_or_else(|| (String::new(), None), |info| (info.name, info.icon));

        let title = if title.is_empty() {
            raw.title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| raw.wm_class.clone())
                .or_else(|| raw.wm_class_instance.clone())
                .unwrap_or_else(|| "Untitled".to_string())
        } else {
            title
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

        debug!(
            "[workspace]  id={} pid={} ws=current title={:?} icon={:?}",
            raw.id, raw.pid, title, icon_name
        );

        result.push(WindowInfo {
            id: raw.id,
            title,
            icon_name,
        });
    }

    debug!("[workspace] {} window(s) passed filter", result.len());

    result.sort_by(|a, b| a.title.cmp(&b.title));
    Some(result)
}

pub async fn activate_window(id: u64) {
    let Ok(conn) = Connection::session().await else {
        return;
    };
    let Ok(windows) = WindowCallsProxy::new(&conn).await else {
        return;
    };

    let result = windows.activate(id as u32).await;
    if let Err(e) = result {
        warn!("[workspace] Activate({id}) failed: {e}");
    }
}

pub async fn close_window(id: u64) {
    let Ok(conn) = Connection::session().await else {
        return;
    };
    let Ok(windows) = WindowCallsProxy::new(&conn).await else {
        return;
    };

    let result = windows.close(id as u32).await;
    if let Err(e) = result {
        warn!("[workspace] Close({id}) failed: {e}");
    }
}

pub async fn close_all_windows(ids: Vec<u64>) {
    for id in ids {
        close_window(id).await;
    }
}
