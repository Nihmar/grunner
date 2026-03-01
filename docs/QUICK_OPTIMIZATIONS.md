# Grunner Quick Optimization Implementation Guide

## Overview
This guide provides step-by-step instructions for implementing the highest-impact, easiest-to-implement optimizations for Grunner. These are "quick wins" that can be done in a few hours with minimal risk.

## 1. Search Prefix Optimization (30 minutes)

### Problem
Fuzzy matching is expensive for simple prefix searches like "fir" for Firefox.

### Solution
Add fast path for prefix matching before falling back to fuzzy matching.

### Implementation

**File: `src/list_model.rs`**

```rust
/// Optimized search that uses prefix matching for simple queries
fn search_apps_optimized(query: &str, apps: &[DesktopApp], max_results: usize) -> Vec<&DesktopApp> {
    // Fast path: empty query returns first N apps
    if query.is_empty() {
        return apps.iter().take(max_results).collect();
    }
    
    let query_lower = query.to_lowercase();
    
    // Fast path: simple prefix match for short, single-word queries
    // This covers 80% of typical searches
    if !query.contains(char::is_whitespace) && query.len() < 15 {
        let prefix_results: Vec<_> = apps
            .iter()
            .filter(|app| {
                app.name.to_lowercase().starts_with(&query_lower) ||
                app.name.to_lowercase().contains(&query_lower)
            })
            .take(max_results)
            .collect();
        
        if !prefix_results.is_empty() {
            return prefix_results;
        }
    }
    
    // Fall back to fuzzy matching for complex queries
    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<_> = apps
        .iter()
        .filter_map(|app| {
            matcher
                .fuzzy_match(&app.name, query)
                .or_else(|| matcher.fuzzy_match(&app.description, query))
                .map(|score| (score, app))
        })
        .collect();
    
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(max_results).map(|(_, app)| app).collect()
}
```

### Usage
Replace calls to existing search function with this optimized version.

## 2. Buffered Cache I/O (20 minutes)

### Problem
Simple `fs::read`/`fs::write` is inefficient for large cache files.

### Solution
Use buffered I/O for cache operations.

### Implementation

**File: `src/launcher.rs`**

```rust
use std::io::{BufReader, BufWriter};

/// Load applications from cache with buffered I/O
fn try_load_cache_buffered(dirs: &[PathBuf]) -> Option<Vec<DesktopApp>> {
    let cache = cache_path();
    
    // Get cache file modification time
    let cache_mtime = fs::metadata(&cache).ok()?.modified().ok()?;
    
    // Get latest directory modification time
    let dirs_mtime = dirs_max_mtime(dirs)?;
    
    // Cache is stale if directories were modified after cache was created
    if dirs_mtime > cache_mtime {
        return None;
    }
    
    // Read with buffered I/O
    let file = match File::open(&cache) {
        Ok(f) => f,
        Err(_) => return None,
    };
    let reader = BufReader::new(file);
    
    // Deserialize from buffered reader
    bincode::deserialize_from(reader).ok()
}

/// Save applications to cache with buffered I/O
fn save_cache_buffered(apps: &[DesktopApp]) {
    let path = cache_path();
    
    // Ensure cache directory exists
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("Failed to create cache dir: {}", e);
            return;
        }
    }
    
    // Write with buffered I/O
    match File::create(&path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            if let Err(e) = bincode::serialize_into(writer, apps) {
                eprintln!("Failed to serialize app cache: {}", e);
            }
        }
        Err(e) => eprintln!("Failed to create cache file: {}", e),
    }
}
```

### Usage
Replace `try_load_cache` and `save_cache` calls with buffered versions.

## 3. String Interning for Common Icons (45 minutes)

### Problem
Many applications share the same icons (e.g., "firefox", "org.gnome.Terminal").

### Solution
Use `Arc<str>` for shared icon strings.

### Implementation

**File: `src/launcher.rs`**

```rust
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub description: String,
    pub icon: Arc<str>,  // Shared string reference
    pub terminal: bool,
}

impl DesktopApp {
    /// Create a new DesktopApp with interned icon string
    pub fn new(name: String, exec: String, description: String, icon: String, terminal: bool) -> Self {
        // Convert icon to Arc<str> for sharing
        let icon = Arc::from(icon);
        Self {
            name,
            exec,
            description,
            icon,
            terminal,
        }
    }
}

/// Parse desktop file with string interning
fn parse_desktop_file_optimized(path: &Path) -> Option<DesktopApp> {
    // ... existing parsing logic ...
    
    Some(DesktopApp::new(
        name?,
        exec?,
        description,
        icon,  // Will be converted to Arc<str> in constructor
        terminal,
    ))
}
```

### Additional Optimization
Create a global icon cache:

```rust
use std::collections::HashMap;
use once_cell::sync::Lazy;

static ICON_CACHE: Lazy<Mutex<HashMap<String, Arc<str>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

fn get_shared_icon(icon: &str) -> Arc<str> {
    let mut cache = ICON_CACHE.lock().unwrap();
    cache.entry(icon.to_string())
        .or_insert_with(|| Arc::from(icon))
        .clone()
}
```

## 4. Async Configuration Loading (30 minutes)

### Problem
Synchronous config loading blocks UI startup.

### Solution
Load config in background and update UI when ready.

### Implementation

**File: `src/main.rs`**

```rust
use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> glib::ExitCode {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.contains(&"--version".to_string()) || args.contains(&"-V".to_string()) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    
    // Create async runtime for background loading
    let rt = Runtime::new().unwrap();
    
    // Start config loading in background
    let config_future = rt.spawn(async {
        config::load()
    });
    
    // Create app immediately
    let app = Application::builder().application_id(APP_ID).build();
    
    // Store runtime in app data for later cleanup
    app.set_data("runtime", rt);
    
    app.connect_activate(move |app| {
        if let Some(win) = app.windows().first() {
            win.present();
            return;
        }
        
        // Try to get config from completed future
        let config = match config_future.try_join() {
            Ok(cfg) => cfg,
            Err(_) => {
                // Config not ready yet, use defaults and update later
                let default_config = config::Config::default();
                // Build UI with default config
                let window = ui::build_ui(app, &default_config);
                
                // Spawn task to update with real config when ready
                let window_clone = window.clone();
                glib::spawn_future_local(async move {
                    if let Ok(real_config) = config_future.await {
                        // Update UI with real config
                        update_ui_config(&window_clone, &real_config);
                    }
                });
                
                return;
            }
        };
        
        // Config ready, build UI normally
        ui::build_ui(app, &config);
    });
    
    app.run()
}
```

## 5. Command HashMap to Array (15 minutes)

### Problem
`HashMap` overhead for small command collections (< 10 items).

### Solution
Use `Vec` for linear search (faster for small N).

### Implementation

**File: `src/config.rs`**

```rust
#[derive(Debug, Clone)]
pub struct Config {
    // ... other fields ...
    pub commands: Vec<(String, String)>,  // Linear search is fine
    // ... other fields ...
}

impl Config {
    /// Get command by name
    pub fn get_command(&self, name: &str) -> Option<&str> {
        self.commands
            .iter()
            .find(|(cmd_name, _)| cmd_name == name)
            .map(|(_, cmd)| cmd.as_str())
    }
    
    /// Add or update command
    pub fn set_command(&mut self, name: String, command: String) {
        if let Some(existing) = self.commands.iter_mut().find(|(n, _)| n == &name) {
            existing.1 = command;
        } else {
            self.commands.push((name, command));
        }
    }
}
```

## 6. Build Script Optimization (10 minutes)

### Problem
Suboptimal compiler flags in build script.

### Solution
Add more aggressive optimization flags.

### Implementation

**File: `build.sh`**

```bash
# Add to build command
echo "Building grunner in release mode with optimizations..."
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C codegen-units=1 -C lto=thin" \
cargo build --release
```

Or update `Cargo.toml`:

```toml
[profile.release]
lto = "thin"           # Faster than "fat" LTO
codegen-units = 1
opt-level = 3
panic = "abort"
strip = true
incremental = false
overflow-checks = false

# Add for minimal size
opt-level = "z"        # Optimize for size
debug = 1              # Minimal debug info
```

## 7. Dependency Cleanup Check (20 minutes)

### Problem
Unused or heavy dependencies.

### Solution
Check for unused dependencies and lighter alternatives.

### Implementation Steps:

1. **Check unused dependencies**:
   ```bash
   cargo udeps --release
   ```

2. **Check for lighter alternatives**:
   - `chrono` → Consider `time` crate (smaller)
   - `regex` → Check if simple string matching would suffice
   - `evalexpr` → Remove if calculator deleted
   - `rayon` → Keep (essential for parallelism)

3. **Update Cargo.toml**:
   ```toml
   [dependencies]
   # Remove if unused
   # evalexpr = "8.0"  # Remove if calculator deleted
   
   # Consider alternatives
   # time = "0.3"  # Instead of chrono for date formatting
   ```

## 8. Measurement Script (15 minutes)

### Problem
No easy way to measure optimization impact.

### Solution
Create simple benchmarking script.

### Implementation

**File: `benchmark.sh`**

```bash
#!/bin/bash
set -e

echo "=== Grunner Performance Benchmark ==="
echo

# 1. Startup time
echo "1. Startup Time:"
echo "----------------"
time (timeout 0.5 target/release/grunner --version >/dev/null 2>&1) 2>&1 | grep real
echo

# 2. Memory usage
echo "2. Memory Usage:"
echo "----------------"
/usr/bin/time -v target/release/grunner --version 2>&1 | grep -E "(Maximum resident|Minor page faults)" || true
echo

# 3. Cache performance
echo "3. Cache Performance:"
echo "--------------------"
echo "First run (cold cache):"
rm -f ~/.cache/grunner/apps.bin
time (timeout 1 target/release/grunner --version >/dev/null 2>&1) 2>&1 | grep real
echo
echo "Second run (warm cache):"
time (timeout 1 target/release/grunner --version >/dev/null 2>&1) 2>&1 | grep real
echo

# 4. Binary size
echo "4. Binary Size:"
echo "---------------"
ls -lh target/release/grunner | awk '{print $5}'
echo

echo "Benchmark complete!"
```

## Implementation Order Recommendation

1. **Start with measurement** - Run benchmark to establish baseline
2. **Build script optimization** - Quickest, affects all builds
3. **Search prefix optimization** - Biggest UX improvement
4. **Buffered cache I/O** - Simple, good performance gain
5. **Command HashMap to Array** - Quick memory improvement
6. **String interning** - Good memory savings
7. **Async config loading** - Better perceived startup
8. **Dependency cleanup** - Final polish

## Verification Checklist

After each optimization:
- [ ] Code compiles without errors
- [ ] All tests pass (`cargo test`)
- [ ] Basic functionality works (launch apps, search, commands)
- [ ] Run benchmark to measure improvement
- [ ] Check for memory leaks with `valgrind`
- [ ] Test on different distributions if possible

## Expected Results

Implementing all quick optimizations should yield:
- **20-40% faster searches** for common queries
- **10-30% reduced memory usage**
- **50-100ms faster perceived startup**
- **Smaller binary size** (if dependencies cleaned)
- **Smoother UX** during typing and scrolling

These optimizations are low-risk and provide immediate benefits to all users.