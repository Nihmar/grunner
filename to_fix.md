error[E0255]: the name `ObsidianAction` is defined multiple times
 --> src/obsidian_item.rs:8:1
  |
5 | pub use ObsidianAction;
  |         -------------- previous import of the type `ObsidianAction` here
...
8 | pub enum ObsidianAction {
  | ^^^^^^^^^^^^^^^^^^^^^^^ `ObsidianAction` redefined here
  |
  = note: `ObsidianAction` must be defined only once in the type namespace of this module
help: you can use `as` to change the binding name of the import
  |
5 | pub use ObsidianAction as OtherObsidianAction;
  |                        ++++++++++++++++++++++

error[E0425]: cannot find function `expand_home` in this scope
   --> src/list_model.rs:296:26
    |
296 |         let vault_path = expand_home(&obs_cfg.vault, &std::env::var("HOME").unwrap_or_default());
    |                          ^^^^^^^^^^^ not found in this scope
    |
note: function `crate::actions::expand_home` exists but is inaccessible
   --> src/actions.rs:15:1
    |
 15 | fn expand_home(path: &str, home: &str) -> PathBuf {
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ not accessible
help: consider importing this function
    |
  1 + use crate::config::expand_home;
    |

error[E0425]: cannot find type `PathBuf` in this scope
   --> src/list_model.rs:342:45
    |
342 |     fn run_find_in_vault(&self, vault_path: PathBuf, pattern: &str) {
    |                                             ^^^^^^^ not found in this scope
    |
help: consider importing this struct
    |
  1 + use std::path::PathBuf;
    |

error[E0425]: cannot find type `PathBuf` in this scope
   --> src/list_model.rs:392:43
    |
392 |     fn run_rg_in_vault(&self, vault_path: PathBuf, pattern: &str) {
    |                                           ^^^^^^^ not found in this scope
    |
help: consider importing this struct
    |
  1 + use std::path::PathBuf;
    |

warning: unused import: `std::path::Path`
  --> src/list_model.rs:16:5
   |
16 | use std::path::Path;
   |     ^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `ObsidianAction`
 --> src/obsidian_item.rs:5:9
  |
5 | pub use ObsidianAction;
  |         ^^^^^^^^^^^^^^

warning: unused import: `ObsidianAction`
  --> src/ui.rs:10:28
   |
10 | use crate::obsidian_item::{ObsidianAction, ObsidianActionItem};
   |                            ^^^^^^^^^^^^^^

error[E0277]: the trait bound `obsidian_item::ObsidianAction: std::default::Default` is not satisfied
  --> src/obsidian_item.rs:20:9
   |
18 |     #[derive(Default)]
   |              ------- in this derive macro expansion
19 |     pub struct ObsidianActionItem {
20 |         pub action: RefCell<ObsidianAction>,
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
   |
help: the trait `std::default::Default` is not implemented for `obsidian_item::ObsidianAction`
  --> src/obsidian_item.rs:8:1
   |
 8 | pub enum ObsidianAction {
   | ^^^^^^^^^^^^^^^^^^^^^^^
help: the trait `std::default::Default` is implemented for `std::cell::RefCell<T>`
  --> /usr/src/debug/rust/rustc-1.93.1-src/library/core/src/cell.rs:1442:1
   = note: required for `std::cell::RefCell<obsidian_item::ObsidianAction>` to implement `std::default::Default`

error[E0616]: field `obsidian_cfg` of struct `AppListModel` is private
   --> src/ui.rs:138:55
    |
138 | ...                   if let Some(cfg) = &model.obsidian_cfg {
    |                                                 ^^^^^^^^^^^^ private field

error[E0616]: field `obsidian_cfg` of struct `AppListModel` is private
   --> src/ui.rs:214:47
    |
214 |                     if let Some(cfg) = &model.obsidian_cfg {
    |                                               ^^^^^^^^^^^^ private field

Some errors have detailed explanations: E0255, E0277, E0425, E0616.
For more information about an error, try `rustc --explain E0255`.
warning: `grunner` (bin "grunner") generated 3 warnings
error: could not compile `grunner` (bin "grunner") due to 7 previous errors; 3 warnings emitted
