[error[E0599]: no method named `spawn_blocking` found for struct `MainContext` in the current scope
   --> src/list_model.rs:135:18
    |
134 |               let output_result = ctx
    |  _________________________________-
135 | |                 .spawn_blocking(move || {
    | |_________________-^^^^^^^^^^^^^^
    |
help: there is a method `spawn_local` with a similar name
    |
135 -                 .spawn_blocking(move || {
135 +                 .spawn_local(move || {
    |

For more information about this error, try `rustc --explain E0599`.
error: could not compile `grunner` (bin "grunner") due to 1 previous error
](arning: unused import: `std::thread::spawn`
 --> src/list_model.rs:4:5
  |
4 | use std::thread::spawn;
  |     ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `glib::idle_add_local_once`
 --> src/list_model.rs:8:5
  |
8 | use glib::idle_add_local_once;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0277]: `Rc<std::cell::Cell<u64>>` cannot be sent between threads safely
   --> src/list_model.rs:131:28
    |
131 |           std::thread::spawn(move || {
    |           ------------------ ^------
    |           |                  |
    |  _________|__________________within this `{closure@src/list_model.rs:131:28: 131:35}`
    | |         |
    | |         required by a bound introduced by this call
132 | |             let output = std::process::Command::new("sh")
133 | |                 .arg("-c")
134 | |                 .arg(&template)
...   |
159 | |             });
160 | |         });
    | |_________^ `Rc<std::cell::Cell<u64>>` cannot be sent between threads safely
    |
    = help: within `{closure@src/list_model.rs:131:28: 131:35}`, the trait `Send` is not implemented for `Rc<std::cell::Cell<u64>>`
note: required because it's used within this closure
   --> src/list_model.rs:131:28
    |
131 |         std::thread::spawn(move || {
    |                            ^^^^^^^
note: required by a bound in `spawn`
   --> /usr/src/debug/rust/rustc-1.93.1-src/library/std/src/thread/functions.rs:125:1

error[E0277]: `*mut c_void` cannot be sent between threads safely
   --> src/list_model.rs:131:28
    |
131 |           std::thread::spawn(move || {
    |  _________------------------_^
    | |         |
    | |         required by a bound introduced by this call
132 | |             let output = std::process::Command::new("sh")
133 | |                 .arg("-c")
134 | |                 .arg(&template)
...   |
159 | |             });
160 | |         });
    | |_________^ `*mut c_void` cannot be sent between threads safely
    |
    = help: the trait `Send` is not implemented for `*mut c_void`
    = note: required for `TypedObjectRef<*mut c_void, ()>` to implement `Send`
note: required because it appears within the type `gtk4::gio::ListStore`
   --> /home/alessandro/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gio-0.21.5/src/auto/list_store.rs:10:16
    |
 10 |     pub struct ListStore(Object<ffi::GListStore, ffi::GListStoreClass>) @implements ListModel;
    |                ^^^^^^^^^
note: required because it's used within this closure
   --> src/list_model.rs:131:28
    |
131 |         std::thread::spawn(move || {
    |                            ^^^^^^^
note: required by a bound in `spawn`
   --> /usr/src/debug/rust/rustc-1.93.1-src/library/std/src/thread/functions.rs:125:1

error[E0277]: `*mut c_void` cannot be shared between threads safely
   --> src/list_model.rs:131:28
    |
131 |           std::thread::spawn(move || {
    |  _________------------------_^
    | |         |
    | |         required by a bound introduced by this call
132 | |             let output = std::process::Command::new("sh")
133 | |                 .arg("-c")
134 | |                 .arg(&template)
...   |
159 | |             });
160 | |         });
    | |_________^ `*mut c_void` cannot be shared between threads safely
    |
    = help: the trait `Sync` is not implemented for `*mut c_void`
    = note: required for `TypedObjectRef<*mut c_void, ()>` to implement `Send`
note: required because it appears within the type `gtk4::gio::ListStore`
   --> /home/alessandro/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gio-0.21.5/src/auto/list_store.rs:10:16
    |
 10 |     pub struct ListStore(Object<ffi::GListStore, ffi::GListStoreClass>) @implements ListModel;
    |                ^^^^^^^^^
note: required because it's used within this closure
   --> src/list_model.rs:131:28
    |
131 |         std::thread::spawn(move || {
    |                            ^^^^^^^
note: required by a bound in `spawn`
   --> /usr/src/debug/rust/rustc-1.93.1-src/library/std/src/thread/functions.rs:125:1

For more information about this error, try `rustc --explain E0277`.
warning: `grunner` (bin "grunner") generated 2 warnings
error: could not compile `grunner` (bin "grunner") due to 3 previous errors; 2 warnings emitted)
