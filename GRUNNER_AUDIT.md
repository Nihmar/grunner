# Grunner — Code Audit

> Sources analyzed: attached zip + README from `github.com/Nihmar/grunner` (main, March 2026)
> Audit author: Claude Sonnet 4.6

---

## Summary

The project is in great shape for a personal Rust/GTK4 application of this scope. The module structure is clear, separation of concerns has improved significantly compared to earlier versions (extraction of `CommandHandler`, the `model/`, `providers/`, `core/`, `ui/` hierarchy), and test coverage for non-GTK code is excellent. The code is readable and well commented.

A few non-trivial refactoring areas remain: a residual god-object (`AppListModel`), a build-time duplication caused by the coexistence of `lib.rs` and `main.rs`, magic numbers scattered through the debounce system, and some simplification opportunities in the subprocess/generation area.

---

## 1. Critical Issue — Double Compilation (`lib.rs` + `main.rs`)

**Files:** `src/lib.rs`, `src/main.rs`

Both `main.rs` and `lib.rs` declare **the same modules** with `mod`:

```rust
// main.rs
mod actions;
mod app_mode;
mod calculator;
// ...

// lib.rs
pub mod actions;
pub mod app_mode;
pub mod calculator;
// ...
```

This causes Cargo to compile **the entire codebase twice** — once as a library crate and once as a separate binary crate — with completely distinct module trees. `main.rs` never uses `grunner::` for anything, so the lib crate exists only for integration tests (if any) or for no real reason at all.

**Recommended fix:** Have `main.rs` use the lib crate:

```rust
// main.rs
use grunner::*; // or selective imports

fn main() -> glib::ExitCode {
    // ... use grunner::logging, grunner::core::config, etc.
}
```

Or, if no lib crate is needed, remove `lib.rs` and declare the binary explicitly in `Cargo.toml`:

```toml
[[bin]]
name = "grunner"
path = "src/main.rs"
```

This cuts compile times in half and removes a source of confusion.

---

## 2. `AppListModel` — Residual God Object

**File:** `src/model/list_model.rs`

Despite the good extraction of `CommandHandler`, `AppListModel` remains a god object with **13 public or `pub(crate)` fields** and methods that touch: scheduling, subprocesses, D-Bus, fuzzy search, Obsidian, and configuration. Specific signals:

### 2a. Too Many Public Fields

```rust
pub store: gio::ListStore,
pub selection: SingleSelection,
pub obsidian_cfg: Option<ObsidianConfig>,
pub(crate) max_results: Cell<usize>,
pub(crate) task_gen: Rc<Cell<u64>>,
pub(crate) active_mode: Rc<Cell<ActiveMode>>,
pub(crate) command_debounce: Rc<RefCell<Option<glib::SourceId>>>,
pub(crate) command_debounce_ms: Cell<u32>,
pub(crate) commands: Rc<RefCell<Vec<CommandConfig>>>,
```

`CommandHandler` accesses these internal fields directly. The "separation" is nominal: `CommandHandler` is a visitor that operates on the model's guts. It would be cleaner to expose an explicit internal API on the model and have `CommandHandler` use only that.

### 2b. Subprocess Methods Embedded in the Model

`run_file_search`, `run_file_grep`, `run_find_in_vault`, `run_rg_in_vault`, and `run_subprocess` logically belong in `providers::subprocess` or a `search::file` module. The model should not know how to construct a `std::process::Command`.

**Suggested refactoring:** extract into `providers::file::FileSearchProvider` (the `SubprocessRunner` infrastructure already exists) implementing `SearchProvider`, or simply expose free functions that return `Vec<String>` via a channel.

### 2c. `run_provider_search` Is Too Long (60+ lines)

It contains timeout logic, channel setup, and polling. It could be decomposed:

```rust
fn run_provider_search(...) {
    let gen = self.bump_task_gen();
    let (tx, rx) = mpsc::channel();
    let timeout = self.setup_clear_timeout(gen, clear_store);
    Self::spawn_provider_query(providers, query, max, tx);
    self.start_provider_poller(rx, gen, terms, timeout, clear_store);
}
```

---

## 3. Code Duplication in `launcher::scan_apps`

**File:** `src/launcher.rs`

```rust
let paths: Vec<PathBuf> = if use_parallel {
    dirs.par_iter()
        .filter(|d| { let exists = d.exists(); ... exists })
        .flat_map(|dir| { WalkDir::new(dir)... })
        .collect()
} else {
    dirs.iter()                        // ← identical except for this
        .filter(|d| { let exists = d.exists(); ... exists })
        .flat_map(|dir| { WalkDir::new(dir)... })
        .collect()
};
```

The two blocks are **identical** apart from `par_iter()` vs `iter()`. Rayon is already optimised for small workloads (it uses the calling thread when the pool is idle). The `use_parallel = dirs.len() > 4` guard is premature optimisation and doubles the code to maintain.

**Fix:** always use `par_iter()`:

```rust
let paths: Vec<PathBuf> = dirs
    .par_iter()
    .filter(|d| { ... })
    .flat_map(|dir| { ... })
    .collect();
```

Same for the parsing block:

```rust
let mut apps: Vec<DesktopApp> = unique_paths
    .par_iter()
    .filter_map(|p| parse_desktop_file(p))
    .collect();
```

---

## 4. Magic Numbers in the Debounce System

**File:** `src/model/list_model.rs`

Multiple debounce values are scattered across the code with no single source of truth:

```rust
const DEFAULT_SEARCH_DEBOUNCE_MS: u32 = 100;  // defined at module level

// but in schedule_populate the comment reads:
// "Default search: 200ms debounce"  ← WRONG, the constant above is 100

// and in schedule_provider_search:
self.schedule_command_with_delay(120, move || { ... });  // ← hardcoded, undocumented

// and in run_provider_search:
glib::timeout_add_local(Duration::from_millis(25), ...)  // ← hardcoded, undocumented
```

**Fix:** collect all timing constants in `core/config.rs` or a dedicated `const` block:

```rust
// Debounce / timing constants
const SEARCH_DEBOUNCE_MS: u32 = 100;
const PROVIDER_DEBOUNCE_MS: u32 = 120;
const PROVIDER_CLEAR_TIMEOUT_MS: u64 = 25;
```

And correct the stale comment (`200ms` → `100ms`).

---

## 5. Duplicated Validation Logic Between `calculator.rs` and `utils.rs`

**Files:** `src/calculator.rs`, `src/utils.rs`

Both `calculator::evaluate` and `utils::is_calculator_result` contain the same allowed-character list:

```rust
// calculator.rs::evaluate
if !expr.chars().all(|c| {
    c.is_ascii_digit() || c == '.' || c == '+' || c == '-' || ...
}) { return None; }

// utils.rs::is_calculator_result
if !expr.chars().all(|c| {
    c.is_ascii_digit() || c == '.' || c == '+' || c == '-' || ...
}) { return false; }
```

The two functions serve different purposes (`evaluate` computes a result, `is_calculator_result` recognises output format), but they share the same character-validation logic. If a new operator is added to `evaluate` (e.g. `!` for factorial), `is_calculator_result` also needs to be updated — and it is easy to forget.

**Fix:** extract a shared predicate:

```rust
// in calculator.rs or utils.rs
pub(crate) fn is_valid_calc_char(c: char) -> bool {
    c.is_ascii_digit()
        || c == '.'
        || matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')')
        || c.is_whitespace()
        || c.is_ascii_alphabetic()
}
```

---

## 6. Inconsistent Task-Generation (Cancellation) Handling

**File:** `src/command_handler.rs`

`handle_file_search` and `handle_file_grep` manage `task_gen` manually:

```rust
fn handle_file_search(&self, arg: &str) {
    let current_gen = self.model.task_gen.get() + 1;
    self.model.task_gen.set(current_gen);
    let model_clone = self.model.clone();
    self.model.schedule_command(move || {
        if model_clone.task_gen.get() == current_gen {  // ← manual check
            model_clone.run_file_search(&arg);
        }
    });
}
```

whereas `handle_obsidian` calls `schedule_command` directly without the check (relying on subprocess internals to handle staleness). This **inconsistency** is a latent bug: a future handler that forgets the check can allow stale results to overwrite fresh ones.

**Fix:** expose a model method that encapsulates bump + check:

```rust
// In AppListModel
pub(crate) fn bump_and_schedule<F>(&self, f: F)
where
    F: FnOnce() + 'static,
{
    let gen = self.bump_task_gen();
    let model_clone = self.clone();
    self.schedule_command(move || {
        if model_clone.task_gen.get() == gen {
            f();
        }
    });
}
```

---

## 7. `config_to_toml` — Local Structs Shadow the Deserialization Structs

**File:** `src/core/config.rs`

```rust
// At the top of the file:
#[derive(Deserialize)]
struct WindowConfig { width: Option<i32>, height: Option<i32> }

// Inside config_to_toml():
#[derive(Serialize)]
struct WindowConfig { width: i32, height: i32 }  // ← same name!
```

Rust allows type shadowing within a function scope, so it compiles, but it is a source of confusion for readers.

**Fix:** rename the inner serialisation structs:

```rust
struct SerWindow { width: i32, height: i32 }
struct SerSearch<'a> { ... }
struct SerTheme { ... }
```

Alternatively, unify serialisation and deserialisation by using `#[serde(skip_serializing_if)]` on the top-level `Config` struct (which already derives `Serialize`).

---

## 8. `item_activation.rs` — Fragile Downcast Chain

**File:** `src/item_activation.rs`

```rust
pub fn activate_item(obj: &glib::Object, ...) {
    if let Some(activatable) = as_activatable(obj) {  // AppItem | CommandItem
        activatable.activate(&ctx);
    } else {
        activate_obsidian_action_item(obj, &ctx);  // tries ObsidianActionItem
        activate_search_result_item(obj, &ctx);    // tries SearchResultItem
    }
}
```

The `else` branch calls both functions unconditionally. If `obj` is an `ObsidianActionItem`, `activate_search_result_item` attempts a downcast, fails silently, and returns. It works by coincidence, but it is not idiomatic.

**Fix:** use a complete enum or explicit pattern matching:

```rust
pub enum GrunnerItem<'a> {
    App(&'a AppItem),
    Command(&'a CommandItem),
    ObsidianAction(&'a ObsidianActionItem),
    SearchResult(&'a SearchResultItem),
}

impl<'a> GrunnerItem<'a> {
    pub fn from_object(obj: &'a glib::Object) -> Option<Self> {
        if let Some(i) = obj.downcast_ref::<AppItem>() { return Some(Self::App(i)); }
        if let Some(i) = obj.downcast_ref::<CommandItem>() { return Some(Self::Command(i)); }
        if let Some(i) = obj.downcast_ref::<ObsidianActionItem>() { return Some(Self::ObsidianAction(i)); }
        if let Some(i) = obj.downcast_ref::<SearchResultItem>() { return Some(Self::SearchResult(i)); }
        None
    }
}
```

---

## 9. `AppListModel::create_factory` Does Not Belong on the Model

**File:** `src/model/list_model.rs`

```rust
pub fn create_factory(&self) -> SignalListItemFactory {
    let active_mode = self.active_mode.get();
    let vault_path = ...;
    crate::ui::list_factory::create_factory(active_mode, vault_path)
}
```

A data model should not create UI widgets. This introduces a conceptual circular dependency (`model → ui`). The factory should be constructed directly in `ui::window::build_main_layout`, passing only the necessary parameters (`active_mode`, `vault_path`).

---

## 10. Minor Observations

### `poll_apps` in `window.rs` Has Too Many Parameters

```rust
fn poll_apps(
    rx: Receiver<Vec<DesktopApp>>,
    model: AppListModel,
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    pinned_strip: GtkBox,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    window: ApplicationWindow,
    dragging: Rc<Cell<bool>>,
)
```

7 parameters. Introduce an `AppLoaderContext` struct or similar.

### `ProviderSearchPoller` in `list_model.rs`

The poller is private to the module but is a heavy struct with 7 fields, and its `poll` method consumes `self` (an interesting pattern). It could live in `providers/dbus/` as part of the D-Bus infrastructure where it conceptually belongs.

### `parse_desktop_file` Does Not Handle `Name[it]=...`

The parser ignores localised name entries (`Name[it]=Gestore file`). On Italian locales, applications display their English name even when a translation is available. This is acceptable as a known limitation but is worth documenting explicitly.

### `AppMode::from_text` Expects Lowercase Input but Does Not Document It

In `ui/window.rs`:
```rust
let text = e.text().to_string().to_lowercase();
let mode = AppMode::from_text(&text);
```

`AppMode::from_text` then uses `starts_with(":ob")` etc., which works because the input is already lowercased. However, `from_text` does not document this precondition. Either add a note in the doc comment, or normalise internally inside `from_text`.

---

## Suggested Refactoring Priority

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 1 | Double lib/bin compilation | Low | High (compile time) |
| 3 | Dedup `scan_apps` parallel blocks | Low | Medium (maintainability) |
| 4 | Named debounce constants + wrong comment | Low | Medium |
| 5 | Shared `is_valid_calc_char` predicate | Low | Low |
| 7 | Rename serialisation structs in `config_to_toml` | Low | Low |
| 6 | Uniform `bump_and_schedule` | Medium | High (correctness) |
| 9 | Move `create_factory` out of model | Medium | Medium |
| 8 | `GrunnerItem` enum for downcasts | Medium | Medium |
| 2b | Extract `FileSearchProvider` | High | High (architecture) |
| 2a | Internal API for `CommandHandler` | High | High (architecture) |
