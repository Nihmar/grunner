use log::info;

/// Open the settings GUI window
///
/// Opens a graphical interface for editing Grunner's configuration settings.
pub fn open_settings(window: &libadwaita::ApplicationWindow, entry: &gtk4::Entry) {
    info!("Opening GUI settings window");
    crate::settings_window::open_settings_window(window, entry);
}
