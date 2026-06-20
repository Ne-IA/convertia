//! convertia-imgworker C-FFI surface — the SINGLE allow-listed `unsafe` module of this crate (G29).
//!
//! Per §2.12.4 every third-party C/C++ decoder runs inside this isolated image-worker subprocess, and
//! per the G29 unsafe policy the worker's `unsafe` is confined to exactly one module path
//! (`crates/imgworker/src/ffi.rs`, pinned in scripts/check-unsafe-policy ALLOWED_UNSAFE_MODULES); the
//! crate root denies it. P1.8 establishes this empty allow-listed placeholder. The
//! libvips/libheif/librsvg `extern "C"` bindings land in P4/P5, each `unsafe` block carrying a
//! `// SAFETY:` justification (G29 requirement 4).

// §2.12.4 / G29: `unsafe` is permitted ONLY here; no FFI bindings exist yet (added in P4/P5).
#![allow(unsafe_code)]
