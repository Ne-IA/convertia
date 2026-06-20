//! `crate::engines` — the §3.2 engine registry + `Engine` trait + selection, the §1.7 generic
//! invocation lifecycle (spawn / progress / cancel / timeout / error-map), and the §3.5 per-engine
//! argument construction. Every spawn routes through `crate::isolation` and the §0.9 pool. Filled by
//! P4.1.
