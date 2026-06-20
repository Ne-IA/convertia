//! `crate::outcome` — the single home of the §2.8 conversion-outcome taxonomy + message catalog and
//! the §2.9 lossy-disclosure catalog, mirrored onto the IPC wire as the §0.4.3 `IpcError` / `ErrorKind`
//! (the §0.7 tier module renamed from `error`; there is no `crate::error`).
//!
//! P1 establishes the module so the §0.7 tree compiles and the §06 drift mechanism has a home; the
//! `ConversionError` taxonomy, the message catalog and the `IpcError` / `ErrorKind` wire types are
//! authored in P2 (§02 owns the outcome strings).
