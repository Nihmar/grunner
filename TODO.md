# Grunner Improvement TODO List

## Priority 1: Code Quality & Safety (Critical)
- [x] **Fix clippy warnings** - 22+ issues across multiple files
  - Removed redundant `libadwaita` import in `src/actions.rs:17`
  - Fixed redundant closure in `src/actions.rs:63`
  - Collapsible if statements in `src/actions.rs:423,466,491,502`
  - Fixed `src/config.rs:184`
  - Fixed `src/search_provider.rs:419, 487, 543, 756`
  - Fixed `src/settings_window/tabs/info.rs:183`
  - Fixed `src/settings_window/tabs/obsidian.rs:87`
  - Fixed `src/settings_window/mod.rs:115`
  - Fixed `src/workspace_bar.rs:214, 262, 270`
  - Fixed `src/items/search_result_item.rs:116` (added allow attribute)
  - Fixed `src/logging.rs:136, 287`
- [x] **Fix unreachable pattern** in `src/list_model.rs:850` - Replaced with proper error handling
- [x] **Fix shell injection vulnerability** in `src/list_model.rs:45` - Removed shell templates, now uses `std::process::Command` with explicit arguments
- [x] **Fix GTK CSS warning** - Removed deprecated `-gtk-overlay-scrolling` property from `src/style.css:289`

## Priority 2: Testing (High)
- [x] **Add unit tests** - Configuration tests added
  - Configuration loading/parsing (`config.rs`) - 8 tests
  - Added tests for default values, TOML parsing, workspace bar enabled
- [x] **Add integration tests** - Created `tests/` directory
  - `tests/config_integration_tests.rs` - 4 integration tests
  - All tests pass

## Priority 3: Performance (Medium)
- [x] **Optimize async runtime** - Standardize to shared runtime pattern
  - Removed per-thread runtime creation in `workspace_bar.rs:412`, now uses shared runtime via `OnceLock`
  - Cached HOME environment variable in `config.rs` and `launcher.rs` to avoid repeated lookups
- [x] **Review Rayon usage** - Added conditional parallelism
  - Only use parallel iteration when directory count > 4 or file count > 50
  - Falls back to sequential processing for small workloads to avoid thread pool overhead

## Priority 4: Architecture & Maintainability (Medium)
- [ ] **Centralize global state** - Consolidate `OnceLock` variables
- [ ] **Standardize error handling** - Replace `unwrap()` with proper error propagation
- [ ] **Improve async patterns** - Consistent usage across modules

## Priority 5: Dependencies & Modernization (Low)
- [ ] **Update dependencies**:
  - `tokio` to latest 1.x (security patches)
  - `zbus` to latest version (check for 6.x breaking changes)
  - `gtk4`/`libadwaita` to latest compatible versions
- [ ] **Use modern Rust features**:
  - `std::env::var_os` instead of `var`
  - `std::path::Path` methods for file operations
  - Async closures (Rust 2024)

## Priority 6: User Experience (Low)
- [ ] **Improve configuration handling** - Atomic file creation to prevent race conditions
- [ ] **Enhance logging** - Add command-line flag for logging configuration
- [ ] **Better error messages** - Replace panics with user-friendly errors

---

## File References for Quick Access

### High Priority Files:
- `src/list_model.rs:850` - Unreachable pattern
- `src/list_model.rs:45` - Shell command execution
- `src/actions.rs:17,63,423,466,491` - Clippy warnings
- `src/workspace_bar.rs:214,262,270,416` - Clippy warnings

### Performance Files:
- `src/workspace_bar.rs:412` - Runtime creation per thread
- `src/config.rs:158` - Repeated HOME env var

### Testing Files to Create:
- `tests/config_tests.rs`
- `tests/search_provider_tests.rs`
- `tests/command_parsing_tests.rs`

---

## Commands to Run

### Fix Clippy Warnings:
```bash
cargo clippy --fix
```

### Run Tests:
```bash
cargo test
cargo test -- --nocapture
```

### Check for Updates:
```bash
cargo outdated
cargo update
```

### Verify Code Quality:
```bash
cargo fmt
cargo clippy
```
