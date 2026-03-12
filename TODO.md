# Grunner Improvement TODO List

## Priority 1: Code Quality & Safety (Critical)
- [ ] **Fix clippy warnings** - 22+ issues across multiple files
  - Collapsible if statements in `src/actions.rs:423,466,491`
  - Redundant closures in `src/actions.rs:63`, `src/main.rs:101`, `src/workspace_bar.rs:416`
  - Manual strip prefix in `src/workspace_bar.rs:262`
- [ ] **Fix unreachable pattern** in `src/list_model.rs:850` - Prevents potential panics
- [ ] **Fix shell injection vulnerability** in `src/list_model.rs:45` - Use `Command` with explicit args

## Priority 2: Testing (High)
- [ ] **Add unit tests** - Zero tests currently exist
  - Configuration loading/parsing (`config.rs`)
  - Search provider discovery
  - Command parsing logic
  - File path utilities
- [ ] **Add integration tests** - Create `tests/` directory

## Priority 3: Performance (Medium)
- [ ] **Optimize async runtime** - Standardize to shared runtime pattern
  - Remove per-thread runtime creation in `workspace_bar.rs:412`
  - Cache filesystem operations in `config.rs:158`
- [ ] **Review Rayon usage** - Consider sequential scanning for small directories

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
