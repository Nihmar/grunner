# To fix

warning: field `desktop_id` is never read
  --> src/search_provider.rs:50:9
   |
47 | pub struct SearchProvider {
   |            -------------- field in this struct
...
50 |     pub desktop_id: String,
   |         ^^^^^^^^^^
   |
   = note: `SearchProvider` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default
