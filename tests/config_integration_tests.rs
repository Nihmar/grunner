//! Integration tests for configuration module

use grunner::core::config::{
    Config, DEFAULT_MAX_RESULTS, DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, default_app_dirs,
};

#[test]
fn test_config_default_values() {
    let config = Config::default();
    assert_eq!(config.window_width, DEFAULT_WINDOW_WIDTH);
    assert_eq!(config.window_height, DEFAULT_WINDOW_HEIGHT);
    assert_eq!(config.max_results, DEFAULT_MAX_RESULTS);
    assert!(config.workspace_bar_enabled);
}

#[test]
fn test_default_app_dirs_count() {
    let dirs = default_app_dirs();
    assert_eq!(dirs.len(), 5);
}

#[test]
fn test_config_path() {
    // This test verifies that config_path returns a valid path
    use grunner::core::config::config_path;
    let path = config_path();
    assert!(path.to_string_lossy().contains("grunner"));
}

#[test]
fn test_workspace_bar_enabled_by_default() {
    let config = Config::default();
    assert!(
        config.workspace_bar_enabled,
        "workspace_bar_enabled should be true by default"
    );
}
