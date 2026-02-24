error[E0308]: mismatched types
   --> src/list_model.rs:75:49
    |
 75 |         let source_id = glib::timeout_add_local(delay_ms, move || {
    |                         ----------------------- ^^^^^^^^ expected `Duration`, found `u32`
    |                         |
    |                         arguments to this function are incorrect
    |
note: function defined here
   --> /home/alessandro/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/glib-0.21.5/src/source.rs:507:8
    |
507 | pub fn timeout_add_local<F>(interval: Duration, func: F) -> SourceId
    |        ^^^^^^^^^^^^^^^^^

error[E0521]: borrowed data escapes outside of method
   --> src/list_model.rs:159:17
    |
 82 |       pub fn populate(&self, query: &str) {
    |                              -----  - let's call the lifetime of this reference `'1`
    |                              |
    |                              `query` is a reference that is only valid in the method body
...
159 | /                 self.schedule_command(300, move || {
160 | |                     model_clone.run_command(cmd_name, &template, &arg);
161 | |                 });
    | |                  ^
    | |                  |
    | |__________________`query` escapes the method body here
    |                    argument requires that `'1` must outlive `'static`

Some errors have detailed explanations: E0308, E0521.
For more information about an error, try `rustc --explain E0308`.
error: could not compile `grunner` (bin "grunner") due to 2 previous errors
