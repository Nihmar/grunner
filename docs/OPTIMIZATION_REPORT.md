# Grunner Optimization Report

## Executive Summary

This report identifies "low-hanging fruit" optimizations for the Grunner application launcher. These are improvements that are relatively easy to implement but could provide meaningful performance benefits, better memory usage, and improved user experience. The analysis is based on code review of version 0.7.0.

## Performance Analysis

### Current Strengths
- **Binary caching** of application list using bincode
- **Parallel processing** with Rayon for file scanning
- **Async operations** for search providers
- **Debounced search** to reduce UI updates
- **LTO enabled** in release builds

### Identified Optimization Opportunities

## 1. Application Loading & Caching

### Issue: Inefficient Cache Invalidation
**Current Implementation**: Cache is invalidated if ANY application directory is newer than cache.
**Problem**: This causes unnecessary cache misses when system directories (like `/usr/share/applications`) are updated, even if user hasn't installed new apps.

**Optimization Suggestion**:
```rust
// Instead of checking all directories, track individual directory mtimes
fn cache_is_valid(dirs: &[PathBuf], cache_mtime: SystemTime) -> bool {
    for dir in dirs {
        if let Ok(dir_mtime) = fs::metadata(dir).and_then(|m| m.modified()) {
            // Only invalidate if this specific directory changed
            if dir_mtime > cache_mtime {
                // Check if changes are relevant (new .desktop files)
                if directory_has_relevant_changes(dir, cache_mtime) {
                    return false;
                }
            }
        }
    }
    true
}
```

**Expected Benefit**: 30-50% reduction in cache invalidations for typical user setups.

### Issue: No Incremental Cache Updates
**Current Implementation**: Entire cache is rebuilt from scratch on invalidation.
**Problem**: Slow when user has many applications (1000+).

**Optimization Suggestion**:
- Store applications per directory in cache
- Only rebuild directories that changed
- Merge with unchanged directories from cache

**Expected Benefit**: 60-80% faster cache rebuilds after system updates.

## 2. Memory Usage & Allocation

### Issue: String Duplication in DesktopApp Parsing
**Current Implementation**: Each `DesktopApp` stores full strings for name, exec, etc.
**Problem**: High memory usage with many applications (typical system: 500-1000 apps).

**Optimization Suggestion**:
```rust
// Use string interning or Arc<str>
pub struct DesktopApp {
    pub name: Arc<str>,           // Shared string reference
    pub exec: Arc<str>,
    pub description: Arc<str>,
    pub icon: Arc<str>,
    pub terminal: bool,
}

// Or use small-string optimization
pub struct CompactDesktopApp {
    pub name: SmallString,        // Inline storage for short strings
    pub exec: SmallString,
    // ...
}
```

**Expected Benefit**: 20-40% memory reduction for application cache.

### Issue: HashMap Overhead for Commands
**Current Implementation**: `HashMap<String, String>` for command configuration.
**Problem**: Hash maps have overhead for small collections (typically < 10 commands).

**Optimization Suggestion**:
```rust
// Use array or smallvec for predictable small collections
pub struct Config {
    // ...
    pub commands: Vec<(String, String)>,  // Linear search is fine for < 10 items
    // Or use phf for compile-time maps
    // pub commands: phf::Map<&'static str, &'static str>,
}
```

**Expected Benefit**: Reduced memory fragmentation, faster lookups.

## 3. Search Performance

### Issue: Fuzzy Matching Overhead
**Current Implementation**: Uses `SkimMatcherV2` for all searches.
**Problem**: Fuzzy matching is expensive for simple prefix searches.

**Optimization Suggestion**:
```rust
fn search_apps(query: &str, apps: &[DesktopApp], max_results: usize) -> Vec<&DesktopApp> {
    // Fast path for empty query
    if query.is_empty() {
        return apps.iter().take(max_results).collect();
    }
    
    // Fast path for simple prefix match
    if !query.contains(char::is_whitespace) && query.len() < 10 {
        let prefix_results: Vec<_> = apps
            .iter()
            .filter(|app| app.name.to_lowercase().starts_with(&query.to_lowercase()))
            .take(max_results)
            .collect();
        
        if !prefix_results.is_empty() {
            return prefix_results;
        }
    }
    
    // Fall back to fuzzy matching
    let matcher = SkimMatcherV2::default();
    // ... existing fuzzy matching logic
}
```

**Expected Benefit**: 5-10x faster searches for common prefix queries.

### Issue: No Search Result Caching
**Current Implementation**: Each search recomputes fuzzy matches.
**Problem**: Users often type incrementally ("f" → "fi" → "fir" → "fire").

**Optimization Suggestion**:
```rust
struct SearchCache {
    // Cache recent search results with LRU eviction
    cache: LruCache<String, Vec<Arc<DesktopApp>>>,
    // Build prefix tree for fast incremental searches
    prefix_tree: Option<PrefixTree>,
}

impl SearchCache {
    fn get_or_compute(&mut self, query: &str) -> Vec<Arc<DesktopApp>> {
        if let Some(cached) = self.cache.get(query) {
            return cached.clone();
        }
        // Compute and cache
        let results = compute_search(query);
        self.cache.put(query.to_string(), results.clone());
        results
    }
}
```

**Expected Benefit**: 50-90% faster incremental typing experience.

## 4. Startup Time Optimization

### Issue: Synchronous Configuration Loading
**Current Implementation**: Config loads synchronously before UI starts.
**Problem**: Blocks UI initialization.

**Optimization Suggestion**:
```rust
async fn load_config_async() -> Config {
    // Load config in background thread
    tokio::task::spawn_blocking(config::load).await.unwrap()
}

// In main():
let config_handle = tokio::spawn(load_config_async());
// Start UI immediately with loading placeholder
// Update UI when config arrives
```

**Expected Benefit**: 100-200ms faster perceived startup.

### Issue: Heavyweight Dependencies Initialization
**Current Problem**: Some dependencies initialize heavy global state on first use.

**Optimization Suggestion**:
```rust
// Initialize expensive dependencies in background
fn prewarm_dependencies() {
    // Initialize regex engine
    let _ = Regex::new("dummy").unwrap();
    // Initialize fuzzy matcher
    let _ = SkimMatcherV2::default();
    // Warm up GTK icon theme cache
    // ...
}

// Call during splash screen or idle time
```

**Expected Benefit**: Smoother first search after startup.

## 5. I/O & System Call Optimization

### Issue: Repeated `fs::metadata` Calls
**Current Implementation**: Multiple metadata calls for cache validation.
**Problem**: System call overhead adds up.

**Optimization Suggestion**:
```rust
// Batch metadata collection
fn collect_dir_metadata(dirs: &[PathBuf]) -> Vec<(PathBuf, SystemTime)> {
    dirs.iter()
        .filter_map(|dir| {
            fs::metadata(dir)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|mtime| (dir.clone(), mtime))
        })
        .collect()
}

// Store in config for reuse
```

**Expected Benefit**: 10-30% faster cache validation.

### Issue: No `O_DIRECT` or Buffered I/O for Cache
**Current Implementation**: Simple `fs::read`/`fs::write` for cache.
**Problem**: Suboptimal for large cache files.

**Optimization Suggestion**:
```rust
use std::io::{BufReader, BufWriter};

fn load_cache_buffered(path: &Path) -> Result<Vec<DesktopApp>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    bincode::deserialize_from(reader)
}

fn save_cache_buffered(path: &Path, apps: &[DesktopApp]) -> Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    bincode::serialize_into(writer, apps)
}
```

**Expected Benefit**: 20-40% faster cache I/O for large systems.

## 6. UI & Rendering Optimizations

### Issue: GTK List View Performance
**Current Problem**: List view rebuilds completely on each search.

**Optimization Suggestion**:
```rust
// Implement incremental updates instead of full rebuilds
fn update_list_incremental(
    store: &gtk4::ListStore,
    old_results: &[DesktopApp],
    new_results: &[DesktopApp]
) {
    // Compute diff between old and new results
    // Only update changed items
    // Use GTK's diff utilities if available
}
```

**Expected Benefit**: Smoother UI updates during typing.

### Issue: Icon Loading Blocking
**Current Problem**: Icons load synchronously during UI updates.

**Optimization Suggestion**:
```rust
// Async icon loading with placeholder
async fn load_icon_async(icon_name: &str) -> Option<gtk4::IconPaintable> {
    gtk4::IconTheme::for_display(&display)
        .lookup_icon_async(icon_name, &[], 32, 1, gtk4::TextDirection::Ltr, gtk4::IconLookupFlags::empty())
        .await
        .ok()
}
```

**Expected Benefit**: Non-blocking UI during icon loading.

## 7. Dependency Optimization

### Issue: Unused or Heavy Dependencies
**Current Dependencies**: Some may be heavier than needed.

**Optimization Opportunities**:
1. **`chrono`**: Consider `time` crate for smaller footprint
2. **`regex`**: Evaluate if all uses need full regex engine
3. **`evalexpr`**: Remove if calculator feature is deleted
4. **`rayon`**: Could use `std::thread` for simpler parallelism

**Expected Benefit**: Smaller binary size, faster compilation.

### Issue: No Feature Flags
**Current Problem**: All features always compiled in.

**Optimization Suggestion**:
```toml
[dependencies]
# Make optional features
obsidian = { package = "urlencoding", version = "2.1", optional = true }
search-provider = { package = "zbus", version = "5.14.0", optional = true }

[features]
default = ["obsidian", "search-provider"]
minimal = []  # Only core functionality
```

**Expected Benefit**: Customizable binary size and dependencies.

## 8. Build System Optimizations

### Issue: Suboptimal Release Profile
**Current Settings**:
```toml
[profile.release]
lto = true
codegen-units = 1
opt-level = 3
panic = "abort"
strip = true
```

**Optimization Suggestions**:
1. Add `opt-level = "z"` for minimal size
2. Consider `codegen-units = 16` for parallel compilation (then `= 1` for final LTO)
3. Add `debug = 1` for minimal debug info
4. Use `-C target-cpu=native` in build script

**Expected Benefit**: 5-15% smaller/faster binary.

## Implementation Priority

### Tier 1: Easy, High Impact (1-2 days work)
1. **Search prefix optimization** - Simple code change, big UX improvement
2. **Buffered cache I/O** - Easy implementation, good performance gain
3. **Async config loading** - Moderate effort, better perceived startup
4. **String interning** - Moderate effort, memory savings

### Tier 2: Moderate Effort, Good ROI (3-5 days)
1. **Incremental cache updates** - Complex but valuable
2. **Search result caching** - Good UX improvement
3. **Dependency cleanup** - Investigation required
4. **Feature flags** - Build system changes

### Tier 3: Larger Refactors (1-2 weeks)
1. **GTK UI optimizations** - Requires deep GTK knowledge
2. **Advanced caching strategies** - Complex state management
3. **Complete architecture review** - Major refactoring

## Measurement & Validation

### Before Implementing:
1. **Baseline measurements**:
   ```bash
   # Startup time
   time grunner --version
   
   # Memory usage
   /usr/bin/time -v grunner --version
   
   # Cache performance
   rm ~/.cache/grunner/apps.bin
   time grunner  # First run
   time grunner  # Cached run
   
   # Search latency
   # Use internal profiling or manual timing
   ```

2. **Profile with**:
   ```bash
   # CPU profiling
   perf record -g target/release/grunner
   perf report
   
   # Memory profiling
   valgrind --tool=massif target/debug/grunner
   ms_print massif.out.*
   
   # Flame graphs
   cargo flamegraph -- target/release/grunner
   ```

### After Each Optimization:
1. Re-run baseline measurements
2. Compare results
3. Ensure no regressions in functionality
4. Document performance improvements

## Risks & Considerations

1. **Complexity vs Benefit**: Some optimizations add complexity for marginal gains
2. **Maintainability**: Optimized code can be harder to understand/maintain
3. **Platform Differences**: Optimizations may work differently across distros
4. **Testing**: Performance changes need thorough testing to avoid regressions
5. **User Configurations**: Optimizations should work across diverse user setups

## Conclusion

Grunner already has a solid performance foundation with caching and parallelism. The identified "low-hanging fruit" optimizations focus on:
1. Reducing unnecessary work (cache invalidation, recomputation)
2. Improving memory efficiency (string handling, data structures)
3. Optimizing common paths (prefix searches, incremental typing)
4. Leveraging async patterns for perceived performance

Implementing even a subset of Tier 1 optimizations could significantly improve the user experience, particularly for users with large application collections or on lower-end hardware.

**Recommended First Steps**:
1. Implement search prefix optimization (fastest ROI)
2. Add buffered cache I/O (simple, effective)
3. Profile actual usage to validate optimization priorities
4. Create benchmarks to track improvements over time

The optimization work should be iterative: measure, implement one change, measure again, and proceed based on actual impact rather than assumed benefits.