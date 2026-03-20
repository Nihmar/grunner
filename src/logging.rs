//! Logging configuration and initialization for Grunner
//!
//! This module provides system-based logging that integrates with:
//! - systemd journal (journald) - primary for Linux desktop environments
//! - syslog - optional fallback (requires the "syslog" Cargo feature)
//! - File-based logging - for debugging and troubleshooting
//! - Standard error - for development environments

use log::{LevelFilter, SetLoggerError};
use simplelog::{Config as SimpleLogConfig, WriteLogger};
use std::fs::{OpenOptions, create_dir_all};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Available logging destinations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogDestination {
    /// systemd journal (preferred for GNOME/Gtk applications)
    Journal,
    /// Traditional syslog
    Syslog,
    /// File-based logging
    File,
    /// Standard error (development)
    Stderr,
    /// No logging
    None,
}

impl Default for LogDestination {
    fn default() -> Self {
        if is_running_under_systemd() {
            LogDestination::Journal
        } else {
            LogDestination::File
        }
    }
}

impl std::fmt::Display for LogDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogDestination::Journal => write!(f, "journal"),
            LogDestination::Syslog => write!(f, "syslog"),
            LogDestination::File => write!(f, "file"),
            LogDestination::Stderr => write!(f, "stderr"),
            LogDestination::None => write!(f, "none"),
        }
    }
}

/// Logging configuration structure
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub destination: LogDestination,
    pub level: LevelFilter,
    pub file_path: Option<PathBuf>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            destination: LogDestination::default(),
            level: LevelFilter::Warn,
            file_path: default_log_file_path(),
        }
    }
}

/// Global configuration storage
static CONFIG: OnceLock<LogConfig> = OnceLock::new();

/// Check if running under systemd
fn is_running_under_systemd() -> bool {
    std::env::var_os("INVOCATION_ID").is_some()
        || std::env::var_os("JOURNAL_STREAM").is_some()
        || std::env::var_os("SYSTEMD_EXEC_PID").is_some()
}

/// Get default log file path
fn default_log_file_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|mut path| {
        path.push("grunner");
        path.push("grunner.log");
        path
    })
}

/// Parse log level from string
pub(crate) fn parse_log_level(level_str: &str) -> LevelFilter {
    match level_str.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Warn,
    }
}

/// Parse log destination from string
pub(crate) fn parse_log_destination(dest_str: &str) -> LogDestination {
    match dest_str.to_lowercase().as_str() {
        "journal" => LogDestination::Journal,
        "syslog" => LogDestination::Syslog,
        "file" => LogDestination::File,
        "stderr" => LogDestination::Stderr,
        "none" => LogDestination::None,
        _ => LogDestination::default(),
    }
}

/// Load configuration from environment variables
fn load_config_from_env() -> LogConfig {
    let destination = std::env::var("GRUNNER_LOG").map_or_else(
        |_| LogDestination::default(),
        |val| parse_log_destination(&val),
    );

    let level =
        std::env::var("GRUNNER_LOG_LEVEL").map_or(LevelFilter::Warn, |val| parse_log_level(&val));

    let file_path = std::env::var("GRUNNER_LOG_FILE")
        .ok()
        .map(PathBuf::from)
        .or_else(default_log_file_path);

    LogConfig {
        destination,
        level,
        file_path,
    }
}

/// Initialize journald logger
#[cfg(feature = "journal")]
fn init_journal_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    use systemd_journal_logger::JournalLog;

    match JournalLog::new() {
        Ok(logger) => {
            // Set the maximum log level first
            log::set_max_level(level);
            // Install the logger
            logger.install()
        }
        Err(e) => {
            eprintln!("Failed to initialize journal logger: {e}, falling back to stderr");
            init_stderr_logger(level)
        }
    }
}

/// Fallback when journal feature is disabled
#[cfg(not(feature = "journal"))]
fn init_journal_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    log::warn!("Journal logging requested but 'journal' feature not enabled");
    init_stderr_logger(level)
}

/// Initialize syslog logger
#[cfg(feature = "syslog")]
fn init_syslog_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    use syslog::{BasicLogger, Facility, Formatter3164};

    let formatter = Formatter3164 {
        facility: Facility::LOG_USER,
        hostname: None,
        process: "grunner".into(),
        pid: std::process::id(),
    };

    let logger = match syslog::unix(formatter) {
        Ok(logger) => logger,
        Err(e) => {
            eprintln!(
                "Failed to initialize syslog logger: {}, falling back to stderr",
                e
            );
            return init_stderr_logger(level);
        }
    };

    match log::set_boxed_logger(Box::new(BasicLogger::new(logger))) {
        Ok(_) => {
            log::set_max_level(level);
            Ok(())
        }
        Err(_) => {
            eprintln!("Failed to register syslog logger, falling back to stderr");
            init_stderr_logger(level)
        }
    }
}

/// Fallback when syslog feature is disabled
#[cfg(not(feature = "syslog"))]
fn init_syslog_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    log::warn!("Syslog logging requested but 'syslog' feature not enabled");
    init_file_logger(level, None)
}

/// Initialize file logger
fn init_file_logger(level: LevelFilter, file_path: Option<&PathBuf>) -> Result<(), SetLoggerError> {
    let path = file_path
        .cloned()
        .or_else(default_log_file_path)
        .unwrap_or_else(|| PathBuf::from("grunner.log"));

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => WriteLogger::init(level, SimpleLogConfig::default(), file),
        Err(e) => {
            eprintln!(
                "Failed to open log file {}: {e}, falling back to stderr",
                path.display()
            );
            init_stderr_logger(level)
        }
    }
}

/// Initialize stderr logger
fn init_stderr_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    let config = SimpleLogConfig::default();
    WriteLogger::init(level, config, std::io::stderr())
}

/// Initialize no-op logger
fn init_no_logger() {
    log::set_max_level(LevelFilter::Off);
}

/// Initialize logging based on configuration
pub fn init_with_config(config: &LogConfig) -> Result<(), SetLoggerError> {
    // Store configuration
    let _ = CONFIG.set(config.clone());

    let result = match config.destination {
        LogDestination::Journal => init_journal_logger(config.level),
        LogDestination::Syslog => init_syslog_logger(config.level),
        LogDestination::File => init_file_logger(config.level, config.file_path.as_ref()),
        LogDestination::Stderr => init_stderr_logger(config.level),
        LogDestination::None => {
            init_no_logger();
            Ok(())
        }
    };

    if let Err(e) = &result {
        eprintln!(
            "Failed to initialize {} logger: {:?}",
            config.destination, e
        );
    }

    result
}

/// Initialize logging based on environment variables
pub fn init() -> Result<(), SetLoggerError> {
    let config = load_config_from_env();
    init_with_config(&config)
}

/// Set up panic hook to capture and log panics
pub fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to log the panic if logging is initialized
        if log::max_level() != LevelFilter::Off {
            let payload = panic_info.payload();
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s
            } else {
                "Box<Any>"
            };

            if let Some(location) = panic_info.location() {
                log::error!(
                    "PANIC at {}:{}: {}",
                    location.file(),
                    location.line(),
                    message
                );
            } else {
                log::error!("PANIC: {message}");
            }
        }

        // Always print to stderr
        eprintln!("PANIC: {panic_info}");

        // Call the default hook
        default_hook(panic_info);
    }));
}

// Check if logging is enabled for a specific level
// pub fn is_enabled(level: Level) -> bool {
//     log::log_enabled!(level)
// }

// Convenience function for expensive debug logging
// pub fn log_if_enabled<F>(level: Level, f: F)
// where
//     F: FnOnce() -> String,
// {
//     if is_enabled(level) {
//         match level {
//             Level::Error => log::error!("{}", f()),
//             Level::Warn => log::warn!("{}", f()),
//             Level::Info => log::info!("{}", f()),
//             Level::Debug => log::debug!("{}", f()),
//             Level::Trace => log::trace!("{}", f()),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_log_level tests ─────────────────────────────────────────

    #[test]
    fn test_parse_log_level_error() {
        assert_eq!(parse_log_level("error"), LevelFilter::Error);
    }

    #[test]
    fn test_parse_log_level_warn() {
        assert_eq!(parse_log_level("warn"), LevelFilter::Warn);
    }

    #[test]
    fn test_parse_log_level_info() {
        assert_eq!(parse_log_level("info"), LevelFilter::Info);
    }

    #[test]
    fn test_parse_log_level_debug() {
        assert_eq!(parse_log_level("debug"), LevelFilter::Debug);
    }

    #[test]
    fn test_parse_log_level_trace() {
        assert_eq!(parse_log_level("trace"), LevelFilter::Trace);
    }

    #[test]
    fn test_parse_log_level_off() {
        assert_eq!(parse_log_level("off"), LevelFilter::Off);
    }

    #[test]
    fn test_parse_log_level_uppercase() {
        assert_eq!(parse_log_level("ERROR"), LevelFilter::Error);
        assert_eq!(parse_log_level("INFO"), LevelFilter::Info);
    }

    #[test]
    fn test_parse_log_level_mixed_case() {
        assert_eq!(parse_log_level("DeBuG"), LevelFilter::Debug);
    }

    #[test]
    fn test_parse_log_level_empty_defaults_to_warn() {
        assert_eq!(parse_log_level(""), LevelFilter::Warn);
    }

    #[test]
    fn test_parse_log_level_invalid_defaults_to_warn() {
        assert_eq!(parse_log_level("banana"), LevelFilter::Warn);
    }

    // ── parse_log_destination tests ───────────────────────────────────

    #[test]
    fn test_parse_log_destination_journal() {
        assert_eq!(parse_log_destination("journal"), LogDestination::Journal);
    }

    #[test]
    fn test_parse_log_destination_syslog() {
        assert_eq!(parse_log_destination("syslog"), LogDestination::Syslog);
    }

    #[test]
    fn test_parse_log_destination_file() {
        assert_eq!(parse_log_destination("file"), LogDestination::File);
    }

    #[test]
    fn test_parse_log_destination_stderr() {
        assert_eq!(parse_log_destination("stderr"), LogDestination::Stderr);
    }

    #[test]
    fn test_parse_log_destination_none() {
        assert_eq!(parse_log_destination("none"), LogDestination::None);
    }

    #[test]
    fn test_parse_log_destination_uppercase() {
        assert_eq!(parse_log_destination("JOURNAL"), LogDestination::Journal);
        assert_eq!(parse_log_destination("STDERR"), LogDestination::Stderr);
    }

    #[test]
    fn test_parse_log_destination_invalid_uses_default() {
        let result = parse_log_destination("invalid");
        // Should return the default destination
        assert_eq!(result, LogDestination::default());
    }

    #[test]
    fn test_parse_log_destination_empty_uses_default() {
        let result = parse_log_destination("");
        assert_eq!(result, LogDestination::default());
    }

    // ── LogDestination Display tests ──────────────────────────────────

    #[test]
    fn test_log_destination_display() {
        assert_eq!(format!("{}", LogDestination::Journal), "journal");
        assert_eq!(format!("{}", LogDestination::Syslog), "syslog");
        assert_eq!(format!("{}", LogDestination::File), "file");
        assert_eq!(format!("{}", LogDestination::Stderr), "stderr");
        assert_eq!(format!("{}", LogDestination::None), "none");
    }
}
