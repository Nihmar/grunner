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

warning: unused import: `ObsidianAction`
  --> src/ui.rs:10:28
   |
10 | use crate::obsidian_item::{ObsidianAction, ObsidianActionItem};
   |                            ^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0425`.
warning: `grunner` (bin "grunner") generated 1 warning
error: could not compile `grunner` (bin "grunner") due to 1 previous error; 1 warning emitted
