//! `crate::detection` — the §1.2 layered content detection (magic-byte sniffing + the bounded
//! pure-Rust structural peeks). The first code to touch untrusted bytes; it runs in-core in safe Rust
//! with no full decode (§2.12.4 — no third-party C/C++ decoder in the trust kernel), so its no-panic
//! discipline is compile-enforced (G4/G14). Filled by P3.26.

// §1.2 in-core untrusted-byte path (T5): indexing/slicing is denied at the module root so an
// out-of-bounds index can never become an in-core panic/DoS. The G4 REQUIRED_ATTRS contract makes
// this deny mandatory the moment this module exists; the rule bites on the real code authored in P3.26.
#![deny(clippy::indexing_slicing)]
