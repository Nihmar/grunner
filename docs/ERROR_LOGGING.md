# Error Logging Implementation Guide for Grunner

## Overview

This guide documents the comprehensive error logging system implemented for Grunner, a GTK4-based application launcher for GNOME. The logging system is designed to provide robust, unobtrusive error tracking that respects user privacy while offering developers essential debugging capabilities.

## Design Goals

1. **System-based Logging**: Logs remain on the user's system and are not transmitted externally
2. **Multiple Backend Support**: Integration with journald (systemd), syslog, and file-based logging
3. **Configurability**: Control log levels and destinations via environment variables
4. **Non-intrusive Operation**: Minimal performance impact on the application
5. **Panic Capture**: Automatic logging of application panics for post-mortem analysis
6. **Graceful Degradation**: Fallback mechanisms when preferred logging backends are unavailable

## Architecture

### Logging System Components

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Code                         │
│    log::error!(), log::warn!(), log::info!(), etc.          │
└────────────────────────────┬────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│                    Logging Module (logging.rs)              │
│  • Configuration loading from environment                   │
│  • Backend initialization (journald/syslog/file/stderr)     │
│  • Panic hook setup                                         │
│  • Error formatting and dispatch                            │
└─────────────────────┬────────────┬──────────────────────────┘
                      │            │
        ┌─────────────▼──┐  ┌──────▼────────────┐
        │  Backend       │  │  Configuration    │
        │  Adapters      │  │  System           │
        │  • journald    │  │  • Env vars       │
        │  • syslog      │  │  • Defaults       │
        │  • file        │  │  • Fallback logic │
        │  • stderr      │  │                   │
        └───────────────┘  └────────────────────┘
```

### Integration with Existing Architecture

The logging system integrates seamlessly with Grunner's existing layered architecture:

- **Presentation Layer**: UI errors logged but not shown to users (unless critical)
- **Application Layer**: Business logic errors, search failures, command execution issues
- **Data Access Layer**: File system errors, D-Bus communication failures, configuration parsing issues

## Implementation Details

### Dependencies

Add the following to `Cargo.toml`:

```toml
[dependencies]
log = "0.4"
simplelog = "0.12"

# Optional features for different backends
systemd-journal = { version = "0.7", optional = true }
syslog = { version = "6.1", optional = true }
dirs = "5.0"  # For cache directory resolution

[features]
default = ["journal"]
journal = ["systemd-journal"]
syslog = ["syslog"]
```

### Core Logging Module

Create `src/logging.rs` with the following structure:

```rust
//! Logging configuration and initialization for Grunner
//!
//! This module provides system-based logging that integrates with:
//! - systemd journal (journald) - primary for Linux desktop environments
//! - syslog - fallback for non-systemd systems
//! - File-based logging - for debugging and troubleshooting
//! - Standard error - for development environments

use log::{LevelFilter, SetLoggerError};
use std::fs::{OpenOptions, create_dir_all};
use std::path::PathBuf;

/// Available logging destinations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogDestination {
    Journal,    // systemd journal (preferred for GNOME/Gtk applications)
    Syslog,     // Traditional syslog
    File,       // File-based logging
    Stderr,     // Standard error (development)
    None,       // No logging
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

impl Default for LogDestination {
    fn default() -> Self {
        if is_running_under_systemd() {
            LogDestination::Journal
        } else {
            LogDestination::File
        }
    }
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
        error!("PANIC: {}", panic_info);
        eprintln!("PANIC: {}", panic_info);
        default_hook(panic_info);
    }));
}
```

### Backend Implementations

#### Journald Backend (Recommended for GNOME)

```rust
#[cfg(feature = "journal")]
fn init_journal_logger(level: LevelFilter) -> Result<(), SetLoggerError> {
    systemd_journal::JournalLog::new()
        .unwrap()
        .filter_level(level)
        .install()
}
```

#### File Backend (Fallback)

```rust
fn init_file_logger(level: LevelFilter, file_path: Option<&PathBuf>) -> Result<(), SetLoggerError> {
    let path = file_path
        .cloned()
        .or_else(default_log_file_path)
        .unwrap_or_else(|| PathBuf::from("grunner.log"));
    
    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }
    
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to open log file {:?}: {}, falling back to stderr", path, e);
            std::io::stderr()
        });
    
    simplelog::WriteLogger::init(level, simplelog::Config::default(), file)
}
```

### Main Application Integration

Update `src/main.rs`:

```rust
mod logging;  // Add this to your module declarations

fn main() -> glib::ExitCode {
    // Initialize logging system
    if let Err(e) = logging::init() {
        eprintln!("Failed to initialize logging: {}", e);
        // Continue without logging
    }
    
    // Set up panic hook to log panics
    logging::setup_panic_hook();
    
    info!("Grunner {} starting up", env!("CARGO_PKG_VERSION"));
    
    // Rest of main function...
}
```

## Usage

### Environment Variables

Control logging behavior with these environment variables:

```bash
# Log destination (journal, syslog, file, stderr, none)
export GRUNNER_LOG=journal

# Log level (error, warn, info, debug, trace)
export GRUNNER_LOG_LEVEL=debug

# Custom log file path (for file logging)
export GRUNNER_LOG_FILE=~/grunner-debug.log
```

### Logging Macros

Use the standard `log` crate macros throughout your code:

```rust
// Error logging (always logged)
error!("Failed to parse configuration: {}", e);

// Warning logging (logged at warn level and above)
warn!("Feature {} is deprecated, use {} instead", old_feature, new_feature);

// Info logging (logged at info level and above)
info!("Application started with config: {:?}", config);

// Debug logging (logged at debug level and above)
debug!("Search query processed in {}ms", duration.as_millis());

// Trace logging (logged at trace level and above)
trace!("Entering function with params: {:?}", params);
```

### Best Practices

1. **Error Context**: Always include context with error messages
   ```rust
   // Bad
   error!("Failed to load file");
   
   // Good
   error!("Failed to load configuration file {}: {}", path.display(), e);
   ```

2. **Structured Logging**: Include relevant metadata
   ```rust
   error!(
       "Search provider {} failed: {} (query: {}, user: {})",
       provider_id, error, query, username
   );
   ```

3. **Performance Sensitive Areas**: Use appropriate log levels
   ```rust
   // Use debug/trace for performance-critical loops
   for item in items.iter() {
       trace!("Processing item: {}", item.id);
       // ... processing logic
   }
   ```

## Viewing Logs

### Journald (Systemd Systems)

```bash
# Follow Grunner logs in real-time
journalctl -f -u grunner

# View all Grunner logs
journalctl -u grunner

# View logs with specific level
journalctl -u grunner --priority=err
journalctl -u grunner --priority=3  # error level

# View logs from specific time period
journalctl -u grunner --since "2024-01-01" --until "2024-01-02"
```

### File Logging

```bash
# Default location
tail -f ~/.cache/grunner/grunner.log

# With custom path
tail -f ~/grunner-debug.log

# Filter by log level
grep -E "(ERROR|WARN)" ~/.cache/grunner/grunner.log
```

### Syslog

```bash
# Traditional syslog location
tail -f /var/log/syslog | grep grunner

# System-specific locations
tail -f /var/log/messages | grep grunner  # RHEL/CentOS
tail -f /var/log/user.log | grep grunner  # Some Debian systems
```

## Integration Examples

### Configuration Loading

Update `src/config.rs`:

```rust
fn apply_toml(content: &str) -> Config {
    let mut cfg = Config::default();

    let toml_cfg: TomlConfig = match toml::from_str(content) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse configuration file: {}", e);
            warn!("Using default configuration due to parse error");
            return cfg;
        }
    };
    
    // ... rest of configuration loading
}
```

### Search Provider Errors

Update `src/search_provider.rs`:

```rust
match outcome {
    Ok(results) if !results.is_empty() => {
        if tx.send(results).is_err() {
            debug!("Search provider channel closed, stopping processing");
            break;
        }
    }
    Err(e) => { 
        error!("Search provider {} error: {}", provider_id, e);
    }
    _ => {
        trace!("Search provider {} returned empty results", provider_id);
    }
}
```

### UI Error Handling

Update `src/ui.rs`:

```rust
fn poll_apps(rx: std::sync::mpsc::Receiver<Vec<AppItem>>, model: glib::WeakRef<AppListModel>) {
    match rx.try_recv() {
        Ok(apps) => {
            info!("Loaded {} applications", apps.len());
            model.upgrade().map(|m| m.set_apps(apps));
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            trace!("Application loading still in progress");
            glib::idle_add_local_once(move || poll_apps(rx, model));
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            error!("Application loading thread terminated unexpectedly");
        }
    }
}
```

## Troubleshooting

### Common Issues

1. **No Logs Appearing**
   - Check environment variables are set correctly
   - Verify log level is not set too high (e.g., `error` won't show `info` messages)
   - Ensure the logging backend is available (journald on systemd systems)

2. **Permission Denied Errors**
   - File logging: Ensure write permissions to log directory
   - Journald: User must have access to systemd journal
   - Syslog: May require elevated privileges

3. **Performance Issues**
   - Reduce log level from `trace` to `debug` or `info`
   - Consider using file logging instead of network-based syslog
   - Use structured logging to minimize string formatting overhead

### Debug Mode

For detailed debugging, use:

```bash
# Enable all logging with trace level
GRUNNER_LOG=stderr GRUNNER_LOG_LEVEL=trace grunner 2>&1 | tee debug.log

# Or for production systems
GRUNNER_LOG=file GRUNNER_LOG_LEVEL=debug GRUNNER_LOG_FILE=/tmp/grunner-debug.log grunner
```

## Security Considerations

1. **No Sensitive Data**: Ensure logs don't contain passwords, API keys, or personal information
2. **Log File Permissions**: File logs should be readable only by the user (0600 permissions)
3. **Journald Security**: Leverages systemd's built-in security features
4. **Environment Variables**: Document that logging configuration is controlled by users

## Performance Impact

The logging system is designed for minimal performance impact:

- **Compile-time Optimization**: Log statements are removed at compile time based on log level
- **Lazy Evaluation**: Use `log::log_enabled!()` macro for expensive computations:
  ```rust
  if log::log_enabled!(log::Level::Debug) {
      let expensive_data = compute_expensive_debug_info();
      debug!("Debug info: {}", expensive_data);
  }
  ```
- **Asynchronous Logging**: Some backends (journald) support asynchronous operation

## Migration from Current Error Handling

Current error handling patterns should be updated:

```rust
// Before
eprintln!("Error: Failed to load config");
// or
if let Err(e) = operation() {
    // Error silently ignored
}

// After
error!("Failed to load configuration: {}", e);
// or
if let Err(e) = operation() {
    warn!("Operation failed, using fallback: {}", e);
    fallback_operation();
}
```

## Conclusion

This error logging implementation provides a robust, flexible system for tracking application behavior while respecting user privacy. By integrating with standard Linux logging infrastructure (journald/syslog) and providing multiple configuration options, it supports both development debugging and production troubleshooting.

The system's design emphasizes:
- **User control** through environment variables
- **System integration** with standard Linux logging
- **Minimal performance impact** through compile-time optimization
- **Comprehensive coverage** of all error paths
- **Privacy preservation** by keeping logs local to the user's system

For ongoing maintenance, regularly review log statements for clarity and relevance, and update documentation as new features are added to the logging system.

---

*Last Updated: Grunner v0.7.0*
*Implementation Status: Proposal*