//! `crate::fs_guard` — the §2.0 no-harm kernel: atomic, exclusive, no-clobber publish on the resolved
//! real file, link-safety + resolved-identity, the frozen source set, path-limit handling and the
//! cross-volume strategy (§2.1 / §2.3 / §2.14). Every output flows through here; engines never write
//! the final file. Filled by P3.1.1.
