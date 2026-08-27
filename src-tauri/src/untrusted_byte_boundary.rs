//! `crate::untrusted_byte_boundary` — the §2.12.4 absolute as an executable, per-`cargo test` assertion
//! (P4.19): **no third-party C/C++ decoder library is linked into or run inside the Rust core — every full
//! decode runs in a separate subprocess** — and the in-core operations on untrusted bytes (the §1.2
//! detection sniffs, the §3.5.6 native CSV/TSV transform) are pure memory-safe Rust.
//!
//! WHY THIS EXISTS. The live enforcers of the absolute are two L(-1) gates: **G53** (`check-core-deps`, the
//! `cargo metadata` closure walk that fails the push when an image-worker C lib reaches the core) and **G29**
//! (`check-unsafe-policy` + the Semgrep SAST: `#![deny(unsafe_code)]` at every crate root, `unsafe` only in the
//! one allow-listed FFI module `crate::platform`). Both fire at pre-push / L4. This module is their
//! **cargo-test-plane companion** — the same defense-in-depth shape as [`crate::no_updater_posture`] (the
//! Cargo-graph side of §7.6.1 behind the G18 ban) and `crate::boot_invariants` (the source side of §0.10
//! behind G47): the absolute is re-asserted inside the crate on every `cargo test`, on all three CI legs, so
//! a regression is caught at the cheapest tier and is readable next to the code it binds. It also carries the
//! §2.12.4 confirmation the P4.19 box names: the image core runs in the SEPARATE image-worker process
//! (`convertia-imgworker`, P4.34/P4.37) — aggregated as its own binary, never linked (§3.6.1).
//!
//! THE FOUR LEGS (each the §2.12.4 text made executable):
//!
//! 1. **The dependency-closure leg** (`[[package]]` rows of the committed workspace `Cargo.lock`, walked from
//!    `convertia-core`): no third-party decoder BINDING is in the closure — the G53 forbidden set (mirrored
//!    verbatim and drift-guarded against the gate script) PLUS the subprocess-only engine families the spec
//!    homes behind the boundary (FFmpeg / poppler / the image codecs, §3.5) PLUS the C compression/XML
//!    backends §2.12.4 replaces with pure Rust; `convertia-imgworker` is NOT in the closure (aggregation, not
//!    linkage); `flate2` rides `miniz_oxide` only (the §0.8 row); and EVERY native-binding-shaped crate the
//!    closure carries is CLASSIFIED in a bijective table (a new `-sys` crate reaching the core is a
//!    conscious, reviewed classification, never a silent link). The lockfile closure is the union over
//!    every target AND every dependency kind (normal + build + dev — a lockfile does not distinguish them),
//!    so it is a SUPERSET of the shipped link set: the leg is identical on all three OS legs and fail-CLOSED
//!    by construction (a dev-only decoder binding in the core's test graph reddens here too — a deliberate
//!    over-strictness; such a crate would be a conscious model extension, never a silent pass).
//! 2. **The registry leg**: over the live §3.2.3 [`crate::engines::engine_registry`], every registered engine's
//!    [`EngineKind`] matches the §3.2.2 table — exhaustively classified per [`EngineId`] (a new variant fails to
//!    compile until it is classified) — and the native CSV/TSV engine is the ONE `InProcessNative` engine.
//! 3. **The detection-source leg** (`src/detection/**`, production code only): the in-core untrusted-byte
//!    module imports ONLY the vetted pure-Rust sniff crates §2.12.4/§0.8 name, and carries no `unsafe`, no
//!    `extern`, no `#[link]`, no process/network path — a pure-Rust FULL decoder (an `image`-class crate) is
//!    refused here too, because the absolute's other half is "no full decode in-core" (§1.2 sniffs only).
//! 4. **The whole-core source leg** (the G29 mirror, scoped to THIS crate — the imgworker's own deny/allow
//!    pair is G29's): `#![deny(unsafe_code)]` at both core-crate roots (`lib.rs` + `main.rs`), no `unsafe`
//!    token and no `allow`/`expect`(… `unsafe_code` …) (any spelling) in any production source of the
//!    crate outside `src/platform/**` — the single allow-listed FFI module (G29 `ALLOWED_UNSAFE_MODULES`).
//!    The source model is strip-FIRST (a `#[cfg(test)]` mentioned in a comment can never forge a gate),
//!    then a PROJECTION that blanks every test-gated ITEM and keeps scanning after it (a statement-level
//!    seam inside a production fn hides nothing behind it — never a first-marker prefix cut); the predicate
//!    is proven ARMED on planted positives, and the real-tree run carries content-based non-vacuity (the
//!    allow-listed module's own `unsafe` is SEEN; a production-line floor; the `ipc/mod.rs` header-mention
//!    and the `fs_guard` statement-seam cases pinned).
//!
//! SCOPE — WHAT "DECODER" MEANS HERE. [Derived-Assumption: P4.19 — the §2.12.4 absolute binds the
//! UNTRUSTED-INPUT path: the §1.2 detection sniffs, the §3.x conversion engines and the §3.5.6 native
//! transform ("the first code touching untrusted bytes" is the section's own scope sentence). The platform
//! WebView host toolkit the §0.4/§0.8 Tauri mandate links — WebKitGTK/GTK (incl. its `gdk-pixbuf` image
//! loader) on Linux, the WebView2 COM shim on Windows, WKWebView on macOS — is the UI HOST, never on that
//! path: the §0.4 IPC carries paths, not file bytes, and the locked §0.10 CSP has no `asset:` protocol, so no
//! dropped file's bytes ever reach the WebView or its loaders. Those bindings are therefore CLASSIFIED (leg 1)
//! rather than forbidden; the §2.12.4 clarification recorded with this box states the same scope in the
//! spec.] The §2.13 `catch_unwind` boundary is deliberately NOT cited by any leg — it catches Rust panics, and
//! only the OS process boundary contains hostile native code (§2.12.4 note).
//!
//! [Build-Session-Entscheidung: P4.19] **Homed at the crate root**, beside `crate::fuzz_replay` /
//! `crate::fuzz_bounds` — the in-core untrusted-byte assertion family — because the legs span the Cargo graph,
//! `crate::engines` and `crate::detection` at once (no single §0.7 tier owns all four), exactly the reason the
//! `test_corpus` / `fuzz_replay` modules give for the same placement. `#[cfg(test)]`-only; adds a FILE, never a
//! directory (G69). Every needle is spelled with `concat!` or lives in a stripped/skipped region so the
//! scanners can never self-match.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::engines::{engine_registry, EngineId, EngineKind};

/// The MIT core crate whose closure is policed — the same package name the G53 walk resolves
/// (`scripts/check-core-deps` `CORE_CRATE_NAME`).
const CORE_CRATE: &str = "convertia-core";

/// The separate image-worker binary (§3.5.5 / §0.7 `crates/imgworker/`) — must NEVER be in the core closure.
const IMAGE_WORKER_CRATE: &str = "convertia-imgworker";

/// The committed workspace `Cargo.lock` (repo root), resolved from this crate's compile-time manifest dir so
/// the walk is CWD-independent (the `no_updater_posture::CARGO_LOCK_PATH` pattern).
const WORKSPACE_LOCK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock");

/// The G53 NEGATIVE fixture's lock (`tests/g53-fixture/`): a planted `convertia-core -> libvips-sys` edge.
/// The armed-canary input for leg 1 — proves the Rust-side walk + stem matcher actually FIRE.
const G53_FIXTURE_LOCK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/g53-fixture/Cargo.lock"
);

/// The G53 gate script — leg 1's forbidden set is drift-guarded against its `FORBIDDEN_STEMS` tuple (read
/// only; the script is L(-1) and never edited from here).
const G53_GATE_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/check-core-deps");

/// This crate's source root — the tree legs 3 and 4 walk.
const CORE_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

/// The two crate roots that must carry the G29 `#![deny(unsafe_code)]` (relative to [`CORE_SRC`]).
const CRATE_ROOTS: &[&str] = &["lib.rs", "main.rs"];

/// The ONE module subtree permitted to carry `unsafe` (G29 `ALLOWED_UNSAFE_MODULES`: `src-tauri/src/platform/*.rs`
/// + `src-tauri/src/platform/**/*.rs`), relative to [`CORE_SRC`].
const ALLOWED_UNSAFE_SUBTREE: &str = "platform";

/// The in-core untrusted-byte module leg 3 scopes to (relative to [`CORE_SRC`]).
const DETECTION_SUBTREE: &str = "detection";

// ─── the forbidden / vetted / classified sets ──

/// The G53 forbidden stems, VERBATIM from `scripts/check-core-deps` `FORBIDDEN_STEMS` (P0.3.7, §3.6.1 / T6):
/// the image-worker-only copyleft C libs + the ImageMagick delegate, matched as name substrings (the same
/// `-sys` / `-rs` / bare-wrapper coverage). `g53_forbidden_stems_mirror_the_gate_script` holds this list
/// IDENTICAL to the gate's tuple, so an owner-acked extension of G53 reddens here until mirrored.
const G53_FORBIDDEN_STEMS: &[&str] = &[
    "libvips",
    "libheif",
    "libde265",
    "de265",
    "librsvg",
    "rsvg",
    "libimagequant",
    "imagequant",
    "imagemagick",
    "magick",
];

/// The §2.12.4 EXTENSION of the G53 set — decoder bindings the spec homes behind the subprocess boundary or
/// replaces with pure Rust, none of which may be linked into the core (name-substring stems, lower-cased):
/// the FFmpeg/libav family (§3.5.1 — a sidecar, never a library link), poppler / MuPDF (§3.5.3), the
/// image-codec C libs behind the image-worker (§3.5.5: libwebp, mozjpeg/libjpeg/turbojpeg, libpng, libavif +
/// dav1d/libaom, OpenJPEG, libtiff, x265), the C zlib backends `flate2` must not select (§2.12.4 names
/// `flate2 rust_backend`/miniz_oxide; the §0.8 row: "NO zlib/zlib-ng C backend"), the C XML parsers the
/// §2.12.4 bounded XML peeks must not use (`quick-xml`/`roxmltree`, entity resolution off), and the C
/// xz/bzip2 decompressors (no in-core C decompressor of any kind). Every stem was checked against the live
/// closure for false positives before landing. [Build-Session-Entscheidung: P4.19]
const SUBPROCESS_ONLY_DECODER_STEMS: &[&str] = &[
    "ffmpeg",
    "libav",
    "poppler",
    "mupdf",
    "libwebp",
    "webp-sys",
    "mozjpeg",
    "libjpeg",
    "turbojpeg",
    "jpeg-sys",
    "libpng",
    "png-sys",
    "libavif",
    "dav1d",
    "libaom",
    "aom-sys",
    "openjpeg",
    "libtiff",
    "tiff-sys",
    "x265",
    "libz-sys",
    "libz-ng-sys",
    "cloudflare-zlib",
    "zlib-sys",
    "libxml",
    "expat",
    "lzma-sys",
    "xz-sys",
    "bzip2-sys",
];

/// The C zlib backends `flate2` MUST NOT be built on (the §0.8 `flate2` row: `rust_backend`/miniz_oxide ONLY,
/// "NO zlib/zlib-ng C backend") — asserted both on `flate2`'s own dependency edge and absent from the whole
/// core closure. `zlib-rs`/`libz-rs-sys` are pure Rust but are NOT the §0.8-selected backend either, so
/// they are refused on the edge as well (the row says miniz_oxide ONLY).
const NON_MINIZ_FLATE2_BACKENDS: &[&str] = &[
    "libz-sys",
    "libz-ng-sys",
    "cloudflare-zlib-sys",
    "zlib-sys",
    "zlib-rs",
    "libz-rs-sys",
];

/// How a native-binding-shaped crate in the core closure is LINKED — the classification leg 1 requires for
/// every such crate, so the reason a C-library binding sits in the MIT core is stated next to its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkClass {
    /// The OS's own platform-API surface (Win32 / CoreFoundation / D-Bus / the tray) — an OS service
    /// binding, not a decoder; no input file's bytes flow through it.
    HostOs,
    /// The §0.4 WebView HOST toolkit the §0.8 Tauri mandate links (GTK/WebKitGTK and its GLib/Pango/Cairo/
    /// gdk-pixbuf/libsoup stack on Linux; the WebView2 COM shim on Windows) — the UI host, never on the
    /// untrusted-input path (§0.4 paths-not-bytes IPC; §0.10 no `asset:` protocol). See the module doc's
    /// `[Derived-Assumption: P4.19]`. This class speaks to the DECODER dimension only: the toolkit's
    /// network-capable members (libsoup, WebKit's HTTP stack) are held to zero egress by the OTHER controls
    /// — the locked §0.10 CSP (no remote origin), the `boot_invariants` no-socket-on-boot scan and the
    /// G42/G42b egress gates — never by this classification.
    WebViewHost,
    /// Tauri's Android (JNI/NDK) arm — `cfg`'d out of every desktop target; lock-present only, never
    /// compiled into a ConvertIA build (§1 desktop-only).
    MobileHost,
    /// The `wasm-bindgen` family — `cfg`'d out of every native target; lock-present only.
    WasmHost,
    /// `-sys`-NAMED but pure Rust (syscall constants / platform-dir lookups) — no C library at all.
    PureRust,
}

/// The BIJECTIVE classification of every native-binding-shaped crate in the `convertia-core` closure (the
/// `is_native_binding_shaped` predicate: `-sys` / `_sys` / `-sys-rs` / `-sys-` names). `every_native_binding_in_the_core_closure_is_classified`
/// holds this table IDENTICAL to the live closure's shaped set in BOTH directions — a crate that appears
/// (a Dependabot bump pulling a new C-library binding into the core) fails until classified here, and a
/// row whose crate left the closure fails until removed (no stale grants that could silently re-admit it).
/// The union over every target (the lockfile is platform-agnostic), so the table — and the assertion —
/// are identical on all three CI legs. [Build-Session-Entscheidung: P4.19]
const CLASSIFIED_NATIVE_BINDINGS: &[(&str, LinkClass)] = &[
    // — Tauri's Android arm (cfg'd out of every desktop target) —
    ("android_log-sys", LinkClass::MobileHost),
    ("jni-sys", LinkClass::MobileHost),
    ("jni-sys-macros", LinkClass::MobileHost),
    ("ndk-sys", LinkClass::MobileHost),
    // — the Linux GTK / WebKitGTK WebView host stack (tauri -> wry -> webkit2gtk) —
    ("atk-sys", LinkClass::WebViewHost),
    ("cairo-sys-rs", LinkClass::WebViewHost),
    // gdk-pixbuf IS a C image loader — linked by the GTK host toolkit for the WebView's own chrome, never
    // handed an input file's bytes (the module-doc scope): classified, not forbidden.
    ("gdk-pixbuf-sys", LinkClass::WebViewHost),
    ("gdk-sys", LinkClass::WebViewHost),
    ("gdkwayland-sys", LinkClass::WebViewHost),
    ("gdkx11-sys", LinkClass::WebViewHost),
    ("gio-sys", LinkClass::WebViewHost),
    ("glib-sys", LinkClass::WebViewHost),
    ("gobject-sys", LinkClass::WebViewHost),
    ("gtk-sys", LinkClass::WebViewHost),
    ("javascriptcore-rs-sys", LinkClass::WebViewHost),
    ("pango-sys", LinkClass::WebViewHost),
    // libsoup is WebKitGTK's HTTP stack — a network-CAPABLE host binding; its egress posture is the
    // §0.10 CSP + G42's, not this table's (see `LinkClass::WebViewHost`).
    ("soup3-sys", LinkClass::WebViewHost),
    ("webkit2gtk-sys", LinkClass::WebViewHost),
    // — the Windows WebView2 COM shim —
    ("webview2-com-sys", LinkClass::WebViewHost),
    // — OS platform-API surfaces —
    ("core-foundation-sys", LinkClass::HostOs),
    ("libappindicator-sys", LinkClass::HostOs),
    ("libdbus-sys", LinkClass::HostOs),
    ("vswhom-sys", LinkClass::HostOs),
    ("windows-sys", LinkClass::HostOs),
    // — the wasm-bindgen family (cfg'd out of every native target) —
    ("js-sys", LinkClass::WasmHost),
    ("web-sys", LinkClass::WasmHost),
    // — `-sys`-named pure Rust —
    ("dirs-sys", LinkClass::PureRust),
    ("linux-raw-sys", LinkClass::PureRust),
];

/// The ONLY third-party crate roots `crate::detection` may reach — the vetted pure-Rust sniff set §2.12.4 /
/// §0.8 name BY NAME: the text-encoding heuristic (`chardetng` + `encoding_rs`, §1.2 step 3, P3.27), the
/// `.svgz` bounded inflate (`flate2` on `miniz_oxide`, §1.2 step 2) and the bounded XML peeks (`quick-xml` /
/// `roxmltree`, entity resolution disabled). A crate a P5–P7 sniff box adds (the OLE2/CFB reader, the ZIP
/// central-directory peek) joins this list WITH its §0.8 row in that box — the list is the spec's set, not a
/// presence assertion, so naming a crate here does not require it to be linked today. Standing obligation
/// carried forward for `quick_xml`: a DIRECT §1.2 `quick-xml` dep must be `>= 0.41` + §0.8-floored (the
/// RUSTSEC-2026-0194/0195 owner-acked ignore covers only the transitive 0.39 dead path).
/// [Build-Session-Entscheidung: P4.19]
const VETTED_DETECTION_CRATES: &[&str] = &[
    "chardetng",
    "encoding_rs",
    "flate2",
    "miniz_oxide",
    "quick_xml",
    "roxmltree",
];

/// Path roots that are never a third-party crate: the std family, the crate-relative keywords, the
/// primitive types (which appear as `str::from_utf8` / `u32::from_le_bytes` path starts) and the lint-TOOL
/// namespaces of `#![deny(clippy::…)]` / `rustdoc::…` attributes (a tool prefix, not a crate).
const NEVER_EXTERNAL_ROOTS: &[&str] = &[
    "std", "core", "alloc", "crate", "super", "self", "bool", "char", "str", "u8", "u16", "u32",
    "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64", "clippy",
    "rustdoc",
];

// ─── the committed-lockfile model ──

/// One `[[package]]` row of a `Cargo.lock` (lockfile v3/v4): `dependencies` entries are `"name"`,
/// `"name version"` or `"name version (source)"` — resolved by [`LockGraph::parse`].
#[derive(Debug, Deserialize)]
struct LockPackage {
    name: String,
    version: String,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// The `Cargo.lock` document — only the `[[package]]` array is modelled (the `version` key + `[metadata]`
/// are irrelevant to the closure walk and ignored by serde).
#[derive(Debug, Deserialize)]
struct LockFile {
    #[serde(default)]
    package: Vec<LockPackage>,
}

/// A `(name, version)` package key — the lockfile's identity (one name may resolve at several versions).
type PkgKey = (String, String);

/// The resolved dependency graph of a `Cargo.lock`: every package key → its resolved dependency keys.
#[derive(Debug)]
struct LockGraph {
    edges: BTreeMap<PkgKey, Vec<PkgKey>>,
}

impl LockGraph {
    /// Parse a committed lockfile and resolve every dependency entry to a concrete `(name, version)` key.
    /// Fails (the test panics with the offending entry) on a dangling or ambiguous entry — a lockfile
    /// cargo itself would refuse — so the walk can never silently skip an edge.
    fn parse(lock_path: &str) -> LockGraph {
        let text = std::fs::read_to_string(lock_path)
            .unwrap_or_else(|e| unreachable_read(lock_path, &e.to_string()));
        let lock: LockFile =
            toml::from_str(&text).unwrap_or_else(|e| unreachable_read(lock_path, &e.to_string()));
        assert!(
            !lock.package.is_empty(),
            "§2.12.4: {lock_path} carries no [[package]] rows — nothing to walk (a truncated lockfile)"
        );
        let mut by_name: BTreeMap<&str, Vec<&LockPackage>> = BTreeMap::new();
        for pkg in &lock.package {
            by_name.entry(pkg.name.as_str()).or_default().push(pkg);
        }
        let mut edges = BTreeMap::new();
        for pkg in &lock.package {
            let mut resolved = Vec::with_capacity(pkg.dependencies.len());
            for entry in &pkg.dependencies {
                let mut parts = entry.split_whitespace();
                let dep_name = parts.next().unwrap_or_default();
                let dep_version = parts.next();
                let candidates = by_name.get(dep_name).map(Vec::as_slice).unwrap_or_default();
                let target = match dep_version {
                    Some(v) => candidates.iter().find(|c| c.version == v),
                    None => {
                        assert!(
                            candidates.len() <= 1,
                            "§2.12.4: {lock_path}: `{}` depends on `{entry}` without a version while {} \
                             versions of `{dep_name}` are locked — an ambiguous edge cargo would refuse",
                            pkg.name,
                            candidates.len()
                        );
                        candidates.first()
                    }
                };
                let target = target.unwrap_or_else(|| {
                    unreachable_read(
                        lock_path,
                        &format!(
                            "`{}` depends on `{entry}`, which no [[package]] row provides",
                            pkg.name
                        ),
                    )
                });
                resolved.push((target.name.clone(), target.version.clone()));
            }
            edges.insert((pkg.name.clone(), pkg.version.clone()), resolved);
        }
        LockGraph { edges }
    }

    /// Every key whose package name is `name` (several when the lock holds multiple versions).
    fn keys_named(&self, name: &str) -> Vec<&PkgKey> {
        self.edges.keys().filter(|(n, _)| n == name).collect()
    }

    /// The transitive dependency closure of the ONE package named `root` (itself included) — a BFS over the
    /// resolved edges; the lockfile is target-agnostic, so this is the union over every cfg / dep kind.
    fn closure_of(&self, root: &str) -> BTreeSet<PkgKey> {
        let roots = self.keys_named(root);
        assert_eq!(
            roots.len(),
            1,
            "§2.12.4: exactly one `{root}` package must be locked (found {})",
            roots.len()
        );
        let mut seen: BTreeSet<PkgKey> = BTreeSet::new();
        let mut queue: VecDeque<PkgKey> = roots.into_iter().cloned().collect();
        while let Some(key) = queue.pop_front() {
            if !seen.insert(key.clone()) {
                continue;
            }
            for dep in self.edges.get(&key).map(Vec::as_slice).unwrap_or_default() {
                if !seen.contains(dep) {
                    queue.push_back(dep.clone());
                }
            }
        }
        seen
    }
}

/// Fail a lockfile read/parse with the path + cause — an `unwrap_or_else` sink (the crate no-panic policy
/// allows `expect`-class failures in `#[cfg(test)]` code; this keeps the message uniform).
fn unreachable_read(path: &str, cause: &str) -> ! {
    let msg = format!("§2.12.4: cannot model {path}: {cause}");
    // The crate-wide `clippy::panic` deny + G8 rule out the panic-macro family; the sanctioned test-code
    // divergence is an `expect` on a `black_box`ed `None` (the `crate::pool` panic-trigger idiom), and an
    // `Infallible` payload lets the empty `match` type the fn as `!`.
    match std::hint::black_box(None::<std::convert::Infallible>).expect(&msg) {}
}

/// The package NAMES of a closure (versions collapsed) — the surface the stem/classification legs read.
fn names_of(closure: &BTreeSet<PkgKey>) -> BTreeSet<&str> {
    closure.iter().map(|(n, _)| n.as_str()).collect()
}

/// The closure crate names matching any forbidden stem (case-insensitive name-substring, the G53 matcher).
fn forbidden_in(names: &BTreeSet<&str>, stems: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            stems.iter().any(|s| lower.contains(s))
        })
        .map(|n| (*n).to_owned())
        .collect()
}

/// The native-binding-shaped-name predicate of the classification leg — the Cargo convention for a
/// C-library binding crate: a `-sys` / `_sys` suffix, the `-sys-rs` GTK-family spelling, or a `-sys-`
/// infix (`jni-sys-macros`). A NAME heuristic (a C link can hide behind any name), so it is the
/// classification RATCHET on top of the stem deny-lists, not the deny-list itself.
fn is_native_binding_shaped(name: &str) -> bool {
    name.ends_with("-sys")
        || name.ends_with("_sys")
        || name.ends_with("-sys-rs")
        || name.contains("-sys-")
}

// ─── the source model ──

/// `src` with string / raw-string / char literals and block + line comments BLANKED (strings first) — the
/// Rust port of `check-unsafe-policy`'s `_strip_rs_noncode` idiom, so a token MENTIONED in a comment, a
/// string or an assertion message never counts. Char literals are recognised by shape (`'x'`, `'\n'`,
/// `'\u{..}'`) so a lifetime tick (`'a`) is left alone and a `'"'` literal cannot open a phantom string.
fn strip_noncode(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let is_ident = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while let Some(c) = at(i) {
        // line comment → drop to end of line (keep the newline so line-based scans stay aligned)
        if c == '/' && at(i + 1) == Some('/') {
            while let Some(n) = at(i) {
                if n == '\n' {
                    break;
                }
                i += 1;
            }
            continue;
        }
        // block comment (nesting honoured) → blank
        if c == '/' && at(i + 1) == Some('*') {
            let mut depth = 0usize;
            while let Some(n) = at(i) {
                if n == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    i += 2;
                } else if n == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if n == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            out.push(' ');
            continue;
        }
        // raw string `r"…"` / `r#"…"#` / `br"…"` (only when `r` is not the tail of an identifier)
        let raw_prefix_ok = !is_ident(at(i.wrapping_sub(1)))
            || (at(i.wrapping_sub(1)) == Some('b') && !is_ident(at(i.wrapping_sub(2))));
        if c == 'r' && raw_prefix_ok {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while at(j) == Some('#') {
                hashes += 1;
                j += 1;
            }
            if at(j) == Some('"') {
                j += 1;
                loop {
                    match at(j) {
                        None => break,
                        Some('"') if (1..=hashes).all(|k| at(j + k) == Some('#')) => {
                            j += 1 + hashes;
                            break;
                        }
                        Some(n) => {
                            if n == '\n' {
                                out.push('\n');
                            }
                            j += 1;
                        }
                    }
                }
                out.push(' ');
                i = j;
                continue;
            }
        }
        // ordinary string → blank (escapes honoured)
        if c == '"' {
            let mut j = i + 1;
            loop {
                match at(j) {
                    // an escape pair is skipped whole — but a backslash-NEWLINE line continuation (this
                    // repo's usual way of wrapping a long assertion message) still has to emit its newline,
                    // or the projection's line numbering drifts against the source (the round-3 review P3)
                    Some('\\') => {
                        if at(j + 1) == Some('\n') {
                            out.push('\n');
                        }
                        j += 2;
                    }
                    None => break,
                    Some('"') => {
                        j += 1;
                        break;
                    }
                    Some(n) => {
                        if n == '\n' {
                            out.push('\n');
                        }
                        j += 1;
                    }
                }
            }
            out.push(' ');
            i = j;
            continue;
        }
        // char literal by shape; anything else starting with `'` is a lifetime tick
        if c == '\'' {
            let close = if at(i + 1) == Some('\\') {
                (i + 3..i + 12).find(|&k| at(k) == Some('\''))
            } else if at(i + 2) == Some('\'') && at(i + 1) != Some('\n') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(close) = close {
                out.push(' ');
                i = close + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `true` iff `code` contains `word` as a whole identifier token (not as a substring of a longer one).
fn has_token(code: &str, word: &str) -> bool {
    let bytes = code.as_bytes();
    let is_ident = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    code.match_indices(word).any(|(start, _)| {
        !is_ident(start.checked_sub(1).and_then(|p| bytes.get(p)))
            && !is_ident(bytes.get(start + word.len()))
    })
}

/// The PRODUCTION projection of one **stripped** source file: every `#[cfg(test)]`-gated ITEM is blanked
/// (newlines kept) and the scan CONTINUES after it — never a cut at the first marker, so this crate's
/// statement-level fault-injection seams (`fs_guard`'s P3.65/P3.19.1
/// `#[cfg(test)] if …` inside production fns, `lib.rs`'s intake seam) never hide the production code that
/// follows them (the round-2 review P1: a first-marker prefix cut left the whole §2.0 publish/plan kernel
/// unscanned). A `#![cfg(test)]` inner attribute makes the whole file test-only (empty projection). The
/// input MUST already be [`strip_noncode`]'d — a marker merely MENTIONED in a doc comment or a string is
/// blanked there, so it can never forge a phantom gate (the round-1 review P1: `ipc/mod.rs` names the
/// marker in its `//!` header; the Loop-memory class `ref-resolver-gate-strip-defs` — locate a structural
/// marker over stripped code, never raw text). Markers are spelled via `concat!` so this file's own text
/// never contains them.
fn production_projection(stripped: &str) -> String {
    if stripped
        .trim_start()
        .starts_with(concat!("#![cfg", "(test)]"))
    {
        return String::new();
    }
    let chars: Vec<char> = stripped.chars().collect();
    let mut out = String::with_capacity(stripped.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if let Some(gate_end) = test_gate_at(&chars, i) {
            let item_end = gated_item_end(&chars, gate_end);
            // blank the attribute + the item it gates, keeping newlines so line counts stay aligned
            out.extend(
                chars
                    .iter()
                    .skip(i)
                    .take(item_end.saturating_sub(i))
                    .map(|&c| if c == '\n' { '\n' } else { ' ' }),
            );
            i = item_end.max(i + 1);
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// If a test-gating `#[cfg(…)]` attribute starts at `i` — `cfg(test)` or an `all(…)` conjunction that
/// names `test` UNCONDITIONALLY (`cfg(all(test, unix))`) — the index just past its closing `]`; otherwise
/// `None`. A `test` reached through a `not(…)` or an `any(…)` — at ANY nesting depth — does not gate:
/// `not(test)` is production, and `any(test, unix)` compiles in a production build on unix, so both keep
/// their item in the scan (over-scanning is the fail-SAFE direction).
fn test_gate_at(chars: &[char], i: usize) -> Option<usize> {
    let head: String = chars.iter().skip(i).take(6).collect();
    if head != "#[cfg(" {
        return None;
    }
    let body_start = i + 6;
    let mut depth = 1usize;
    let mut j = body_start;
    while let Some(&c) = chars.get(j) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    let body: String = chars
        .iter()
        .skip(body_start)
        .take(j.saturating_sub(body_start))
        .collect();
    // whitespace-COMPACTED before classification, so the rule cannot be defeated by spacing
    // (`all(not( test ), unix)`; the round-3 review P2 — the inclusion check was token-based while the
    // `not` exclusion was an exact substring)
    let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let is_gate = body == "test" || (body.starts_with("all(") && names_test_unconditionally(&body));
    if !is_gate {
        return None;
    }
    // `)` then `]`
    let close = j + 1;
    (chars.get(close) == Some(&']')).then_some(close + 1)
}

/// `true` iff the whitespace-compacted `cfg` body names the `test` predicate at least once with every
/// enclosing scope a CONJUNCTION — i.e. the attribute really does gate test-only code. A `test` reached
/// through a `not(…)` (`all(not(test), unix)`, `all(not(all(test)), unix)` — production-only) or through
/// an `any(…)` (`all(any(test, unix), windows)` — compiled in a production build on unix) is NOT gating
/// and must not blank its item; the same holds at any nesting depth, because a scope of either kind
/// anywhere above the token breaks the implication "this item exists only under `cfg(test)`". Scope-aware
/// rather than substring-based (the round-3 review P2; the `any` half is the round-4 review P2 — the
/// sibling clause the `not` fix first left open). Anything this cannot confirm keeps its item in the scan
/// (over-scanning is the fail-SAFE direction).
fn names_test_unconditionally(compact: &str) -> bool {
    let bytes = compact.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut scopes: Vec<String> = Vec::new();
    let mut ident = String::new();
    for (i, c) in compact.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(c);
            match bytes.get(i + 1) {
                // still inside the identifier, or it opens a scope the `(` arm will push
                Some(&next) if is_ident(next) || next == b'(' => continue,
                _ => {}
            }
            if ident == "test" && !scopes.iter().any(|s| s == "not" || s == "any") {
                return true;
            }
            ident.clear();
        } else if c == '(' {
            scopes.push(std::mem::take(&mut ident));
        } else {
            if c == ')' {
                scopes.pop();
            }
            ident.clear();
        }
    }
    false
}

/// The index just past the ITEM a gate attribute ending at `from` applies to — every further outer
/// attribute is skipped, then — at paren/bracket depth 0 — the first `{` opens a balanced block that is
/// skipped whole (a `mod`/`fn`/`impl`/statement-level `if` seam), or the first `;` ends a declaration
/// (`mod x;`, `use …;`, `let …;`, `struct X;`). Strings/comments are already blanked, so every brace is real.
fn gated_item_end(chars: &[char], from: usize) -> usize {
    let mut i = from;
    loop {
        while chars.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        if chars.get(i) == Some(&'#') && chars.get(i + 1) == Some(&'[') {
            let mut depth = 0usize;
            while let Some(&c) = chars.get(i) {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            continue;
        }
        break;
    }
    let mut depth = 0usize;
    while let Some(&c) = chars.get(i) {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => return i + 1,
            '{' if depth == 0 => {
                let mut braces = 0usize;
                while let Some(&b) = chars.get(i) {
                    match b {
                        '{' => braces += 1,
                        '}' => {
                            braces -= 1;
                            if braces == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                return chars.len();
            }
            _ => {}
        }
        i += 1;
    }
    chars.len()
}

/// Every `#[cfg(test)]`-declared FILE module (`#[cfg(test)]` immediately followed by `mod x;` in a
/// declaring file, located over STRIPPED text) anywhere under `root`, resolved to the file paths Rust
/// binds the declaration to — relative to `root`: `dir/x.rs` and `dir/x/mod.rs`, where `dir` is the
/// declaring file's own directory for a `lib.rs` / `main.rs` / `mod.rs` and `<declaring-stem>/` for any
/// other file (the 2018 module-file rules). These test-only sources (`test_corpus`, `fuzz_replay`, this
/// module, …) carry NO production code and are skipped whole by the source legs; binding the skip to
/// the declaring DIRECTORY (not a bare stem) means a production file that merely shares a stem elsewhere
/// in the tree is never skipped (the round-1 review P3).
fn test_only_file_modules(root: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    let marker = concat!("#[cfg", "(test)]");
    for (path, text) in rs_files(root) {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let declaring_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let parent = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        let dir = if matches!(declaring_stem, "lib" | "main" | "mod") {
            parent
        } else {
            parent.join(declaring_stem)
        };
        let stripped = strip_noncode(&text);
        for (idx, _) in stripped.match_indices(marker) {
            let rest = stripped
                .get(idx + marker.len()..)
                .unwrap_or_default()
                .trim_start();
            let Some(decl) = rest.strip_prefix("mod ") else {
                continue;
            };
            let name: String = decl
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if decl
                .get(name.len()..)
                .is_some_and(|tail| tail.trim_start().starts_with(';'))
            {
                files.insert(dir.join(format!("{name}.rs")));
                files.insert(dir.join(&name).join("mod.rs"));
            }
        }
    }
    files
}

/// `true` iff `code` carries an `allow` — or an `expect`, the other lint-level attribute that re-permits
/// a denied lint — whose parenthesised body names the `unsafe_code` lint, in ANY spelling
/// (`#[allow(unsafe_code)]`, `#![allow( unsafe_code )]`, `#[allow(unsafe_code, dead_code)]`,
/// `#[allow(dead_code, unsafe_code)]`, the `expect` form): the keyword is found at an identifier boundary,
/// its parenthesised body taken to the matching `)`, and `unsafe_code` matched as a whole token inside
/// it — the shape of G29's `_ALLOW_RE` + `_UNSAFE_TOKEN` pair, widened by the `expect` attribute (the
/// round-1 review P1's exact-spelling weakness; the round-2 P3). A production `Option`/`Result` unwrap
/// METHOD of the same name never matches: its body is a message, never the `unsafe_code` token.
fn has_allow_unsafe(code: &str) -> bool {
    ["allow", "expect"]
        .iter()
        .any(|keyword| has_lint_permit(code, keyword, "unsafe_code"))
}

/// `true` iff `code` carries `<keyword>( … <lint> … )` with `keyword` at an identifier boundary and `lint`
/// as a whole token inside the balanced parenthesised body — ANY attribute position.
fn has_lint_permit(code: &str, keyword: &str, lint: &str) -> bool {
    lint_attr_names_lint(code, keyword, lint, false)
}

/// [`has_lint_permit`] restricted to the CRATE-INNER attribute FORM `#![<keyword>(…)]` — the form G29's
/// own `_DENY_RE` (`^\s*#!\[\s*(deny|forbid)\s*\(`) requires of a crate root, matched as a strict SUPERSET
/// of that regex: this accepts whitespace between `#`, `!` and `[` and does not require line start, both
/// of which rustc tolerates and the regex does not. The divergence is fail-LOUD in one direction only —
/// such a root would be green here and RED at G29's push-time gate, never the reverse — so it can cost a
/// visible re-spelling, never a silent pass (the round-6 review P3). Without the anchor an item-level
/// `#[deny(unsafe_code)] mod m {}` or a `cfg_attr`-wrapped deny would satisfy a crate-root assertion that
/// G29 itself would red (the round-5 review finding — the anchor the pre-round-4 exact-string check
/// carried). Residual at PARITY with `_DENY_RE`, recorded rather than closed: neither this nor the regex
/// distinguishes a crate-inner attribute from a module-inner `mod m { #![deny(…)] }`.
fn has_inner_lint_attr(code: &str, keyword: &str, lint: &str) -> bool {
    lint_attr_names_lint(code, keyword, lint, true)
}

/// `true` iff the char sequence immediately before `keyword_start` is the inner-attribute opener `#![`
/// (whitespace between the three tokens tolerated, as rustc does).
fn opens_inner_attribute(code: &str, keyword_start: usize) -> bool {
    let mut before = code
        .get(..keyword_start)
        .unwrap_or_default()
        .chars()
        .rev()
        .filter(|c| !c.is_whitespace());
    before.next() == Some('[') && before.next() == Some('!') && before.next() == Some('#')
}

/// The shared scan behind [`has_lint_permit`] / [`has_inner_lint_attr`].
fn lint_attr_names_lint(code: &str, keyword: &str, lint: &str, inner_only: bool) -> bool {
    let bytes = code.as_bytes();
    let is_ident = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    code.match_indices(keyword).any(|(start, _)| {
        if is_ident(start.checked_sub(1).and_then(|p| bytes.get(p))) {
            return false;
        }
        if inner_only && !opens_inner_attribute(code, start) {
            return false;
        }
        let after = code.get(start + keyword.len()..).unwrap_or_default();
        let Some(body) = after.trim_start().strip_prefix('(') else {
            return false;
        };
        let mut depth = 1usize;
        let end = body
            .char_indices()
            .find(|(_, c)| {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map_or(body.len(), |(i, _)| i);
        has_token(body.get(..end).unwrap_or_default(), lint)
    })
}

/// The G29 violations one production source carries (empty = clean): an `unsafe` token or an
/// `allow`/`expect`(… `unsafe_code` …) outside the allow-listed `platform/` subtree. `code` is a STRIPPED
/// production projection (see [`production_sources`]). Pure, so the planted-positive fixture test below proves each
/// violation shape actually FIRES — the leg is armed, not file-count-vacuous.
fn unsafe_policy_violations(rel: &Path, code: &str) -> Vec<String> {
    let mut found = Vec::new();
    if under(rel, ALLOWED_UNSAFE_SUBTREE) {
        return found;
    }
    if has_token(code, "unsafe") {
        found.push(format!(
            "{} carries an `unsafe` token outside the allow-listed `src/{ALLOWED_UNSAFE_SUBTREE}/` FFI module",
            rel.display()
        ));
    }
    if has_allow_unsafe(code) {
        found.push(format!(
            "{} re-permits `unsafe_code` outside the allow-listed `src/{ALLOWED_UNSAFE_SUBTREE}/` FFI module",
            rel.display()
        ));
    }
    found
}

/// Every `.rs` file under `root` (recursive, sorted) with its full text.
fn rs_files(root: &Path) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let entries = std::fs::read_dir(dir).expect("the crate src dir is readable");
        for entry in entries {
            let path = entry.expect("a src dir entry is readable").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file is readable");
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

/// Every production source file under `root`, as `(path relative to root, stripped production code)`:
/// test-only file modules skipped whole; every other file is comment/string-STRIPPED FIRST and then run
/// through [`production_projection`], which blanks each `#[cfg(test)]`-gated item and keeps scanning after
/// it (strip first, project second — a marker inside a comment or string can never forge a gate, and a
/// statement-level seam never blinds the production code behind it).
fn production_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let test_only = test_only_file_modules(root);
    rs_files(root)
        .into_iter()
        .filter_map(|(path, text)| {
            let rel = path
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .unwrap_or(path);
            if test_only.contains(&rel) {
                return None;
            }
            let stripped = strip_noncode(&text);
            Some((rel, production_projection(&stripped)))
        })
        .collect()
}

/// `true` iff `rel` is under the `subtree` directory (the first path component).
fn under(rel: &Path, subtree: &str) -> bool {
    rel.components()
        .next()
        .is_some_and(|c| c.as_os_str() == subtree)
}

/// The external crate ROOTS one stripped production source reaches: the first segment of every `use` path
/// plus every lower-case path START — `ident::…` at the head of a path, INCLUDING the head of a global
/// `::ident::…` path (a `::` is a mid-path separator only when an identifier, a `>` or a `)` precedes it; a
/// `::` after whitespace / `(` / `=` / `,` / `&` is the global-path marker, so `::image::io::Reader` reports
/// `image` — the round-1 review P1) — that is not a `use`-bound name, a local `mod`, or a never-external
/// root. Fail-closed by construction: an unrecognised lower-case path start is reported as an external root
/// and must be vetted.
fn external_roots(code: &str) -> BTreeSet<String> {
    let mut bound: BTreeSet<String> = BTreeSet::new();
    let mut roots: BTreeSet<String> = BTreeSet::new();
    // `use` statements + local `mod` declarations, statement by statement (a `;`-chunk may open with
    // attributes / a doc-stripped blank run before the keyword, so keywords are found at an identifier
    // boundary anywhere in the chunk, never only at its start)
    for stmt in code.split(';') {
        if let Some(name) = after_keyword(stmt, "mod ") {
            bound.insert(ident_prefix(name));
        }
        let Some(path) = after_keyword(stmt, "use ") else {
            continue;
        };
        let path = path.trim().trim_start_matches("::");
        roots.insert(ident_prefix(path));
        let (prefix, leaves) = match path.split_once('{') {
            Some((p, rest)) => (p.trim_end_matches("::"), rest.trim_end_matches('}')),
            None => ("", path),
        };
        for leaf in leaves.split(',') {
            let leaf = leaf.trim().trim_matches('}').trim();
            if leaf.is_empty() || leaf == "*" {
                continue;
            }
            let name = match leaf.split_once(" as ") {
                Some((_, alias)) => alias.trim().to_owned(),
                None if leaf == "self" => prefix.rsplit("::").next().unwrap_or_default().to_owned(),
                None => leaf.rsplit("::").next().unwrap_or_default().to_owned(),
            };
            bound.insert(name);
        }
    }
    // inline path starts
    let bytes = code.as_bytes();
    for (start, _) in code.match_indices("::") {
        let mut b = start;
        while b > 0
            && bytes
                .get(b - 1)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            b -= 1;
        }
        let ident = code.get(b..start).unwrap_or_default();
        // a `::` right before the identifier is a mid-path SEPARATOR only when an identifier, a `>`
        // (`Vec::<u8>::new`, `<T as Trait>::f`) or a `)` precedes it; otherwise it is the leading
        // global-path marker and the identifier IS the path's root
        let preceded_by_path_sep = b >= 2
            && code.get(b - 2..b) == Some("::")
            && b >= 3
            && bytes
                .get(b - 3)
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'>' | b')'));
        if ident.is_empty()
            || preceded_by_path_sep
            || !ident.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        {
            continue;
        }
        if NEVER_EXTERNAL_ROOTS.contains(&ident) || bound.contains(ident) {
            continue;
        }
        roots.insert(ident.to_owned());
    }
    roots
        .into_iter()
        .filter(|r| !NEVER_EXTERNAL_ROOTS.contains(&r.as_str()))
        .collect()
}

/// The text after the FIRST identifier-boundary occurrence of the keyword `kw` (e.g. `"use "`) in `stmt` —
/// `None` when the keyword only appears as the tail of a longer identifier (`reuse `) or not at all.
fn after_keyword<'a>(stmt: &'a str, kw: &str) -> Option<&'a str> {
    let bytes = stmt.as_bytes();
    stmt.match_indices(kw).find_map(|(idx, _)| {
        let at_boundary = idx == 0
            || bytes
                .get(idx - 1)
                .is_some_and(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        at_boundary.then(|| stmt.get(idx + kw.len()..).unwrap_or_default())
    })
}

/// The leading identifier of `s` (the first path segment).
fn ident_prefix(s: &str) -> String {
    s.trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

// ─── leg 1: the dependency closure ──

// §2.12.4 / §6.4.1 unit (G15): the armed canary — the Rust-side lock walk + stem matcher FIRE on the G53
// negative fixture (a planted `convertia-core -> libvips-sys` edge), so the real-tree assertions below can
// never pass by a broken walker (the G24 planted-positive discipline applied to this module).
#[test]
fn the_lock_walk_is_armed_on_the_planted_g53_fixture() {
    let graph = LockGraph::parse(G53_FIXTURE_LOCK);
    let closure = graph.closure_of(CORE_CRATE);
    let hits = forbidden_in(&names_of(&closure), G53_FORBIDDEN_STEMS);
    assert_eq!(
        hits,
        vec!["libvips-sys".to_owned()],
        "§2.12.4/G53: the planted fixture edge must be found by the in-crate walk"
    );
    assert!(
        is_native_binding_shaped("libvips-sys"),
        "§2.12.4: the classification predicate must recognise the planted `-sys` binding"
    );
}

// §2.12.4 / §6.4.1 unit (G15): the G53 forbidden set is mirrored VERBATIM — the `FORBIDDEN_STEMS` tuple
// of `scripts/check-core-deps` (read only; L(-1)) and this module's list are identical, so an owner-acked
// extension of the gate reddens here until mirrored (the two enforcers never drift apart silently).
#[test]
fn g53_forbidden_stems_mirror_the_gate_script() {
    let script = std::fs::read_to_string(G53_GATE_SCRIPT)
        .expect("§2.12.4/G53: the check-core-deps gate script is tracked at the repo root");
    let tuple = script
        .split_once("FORBIDDEN_STEMS = (")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(body, _)| body)
        .expect("§2.12.4/G53: check-core-deps declares a `FORBIDDEN_STEMS = (...)` tuple");
    let gate_stems: BTreeSet<&str> = tuple.split('"').skip(1).step_by(2).collect();
    let mirror: BTreeSet<&str> = G53_FORBIDDEN_STEMS.iter().copied().collect();
    assert_eq!(
        mirror, gate_stems,
        "§2.12.4/G53: G53_FORBIDDEN_STEMS must equal the gate's FORBIDDEN_STEMS tuple (mirror it)"
    );
}

// §2.12.4 / §6.4.1 unit (G15): the real closure — no third-party decoder binding reaches `convertia-core`:
// neither the G53 image-worker set nor the §2.12.4 extension (FFmpeg/poppler/the image codecs, the C
// zlib/XML/xz/bzip2 backends). The lockfile is the union over every target and dep kind, so the closure is
// a SUPERSET of any shipped build's — the strongest form of the assertion.
#[test]
fn no_third_party_decoder_binding_reaches_the_core_closure() {
    let graph = LockGraph::parse(WORKSPACE_LOCK);
    let closure = graph.closure_of(CORE_CRATE);
    let names = names_of(&closure);
    assert!(
        names.len() > 100,
        "§2.12.4: the core closure walk must reach the real graph (found only {} crates)",
        names.len()
    );
    let g53_hits = forbidden_in(&names, G53_FORBIDDEN_STEMS);
    assert!(
        g53_hits.is_empty(),
        "§2.12.4/§3.6.1 (T6): an image-worker C lib reached the MIT core closure: {g53_hits:?}"
    );
    let ext_hits = forbidden_in(&names, SUBPROCESS_ONLY_DECODER_STEMS);
    assert!(
        ext_hits.is_empty(),
        "§2.12.4: a subprocess-only decoder / C backend binding reached the core closure: {ext_hits:?}"
    );
}

// §2.12.4 / §3.6.1 / §6.4.1 unit (G15): the image core is AGGREGATED, never linked — the separate
// `convertia-imgworker` binary exists as its own locked workspace member (non-vacuity) and is NOT in the
// core's closure; the P4.34/P4.37 image-worker route through the §2.12 boundary can never become a link.
#[test]
fn the_image_worker_is_aggregated_never_linked() {
    let graph = LockGraph::parse(WORKSPACE_LOCK);
    assert_eq!(
        graph.keys_named(IMAGE_WORKER_CRATE).len(),
        1,
        "§3.5.5/§0.7: the image-worker is a locked workspace member (the separate externalBin crate)"
    );
    let closure = graph.closure_of(CORE_CRATE);
    assert!(
        !names_of(&closure).contains(IMAGE_WORKER_CRATE),
        "§2.12.4/§3.6.1: `{IMAGE_WORKER_CRATE}` must never be in the `{CORE_CRATE}` dependency closure — \
         it is aggregated as its own binary, not linked into the MIT core"
    );
}

// §2.12.4 / §0.8 / §6.4.1 unit (G15): `flate2` inflates on the pure-Rust `miniz_oxide` backend ONLY — its
// own dependency edge carries `miniz_oxide` and none of the C zlib / zlib-ng / non-miniz backends, and no C
// zlib binding is anywhere in the core closure. `flate2` is present by the §0.8 row ("pinned, in lockfile"),
// so the edge assertion is non-vacuous.
#[test]
fn flate2_inflates_on_the_pure_rust_backend_only() {
    let graph = LockGraph::parse(WORKSPACE_LOCK);
    let closure = graph.closure_of(CORE_CRATE);
    let flate2_keys: Vec<&PkgKey> = closure.iter().filter(|(n, _)| n == "flate2").collect();
    assert!(
        !flate2_keys.is_empty(),
        "§0.8: `flate2` is a pinned §1.2 crate in the core closure (the .svgz bounded inflate)"
    );
    for key in flate2_keys {
        let deps: BTreeSet<&str> = graph
            .edges
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert!(
            deps.contains("miniz_oxide"),
            "§0.8/§2.12.4: flate2 {} must ride `miniz_oxide` (deps: {deps:?})",
            key.1
        );
        let non_miniz: Vec<&&str> = deps
            .iter()
            .filter(|d| NON_MINIZ_FLATE2_BACKENDS.contains(d))
            .collect();
        assert!(
            non_miniz.is_empty(),
            "§0.8/§2.12.4: flate2 {} selects a non-miniz backend: {non_miniz:?}",
            key.1
        );
    }
    let c_zlib: Vec<String> = forbidden_in(
        &names_of(&closure),
        &["libz-sys", "libz-ng-sys", "cloudflare-zlib-sys", "zlib-sys"],
    );
    assert!(
        c_zlib.is_empty(),
        "§2.12.4: a C zlib binding is in the core closure: {c_zlib:?}"
    );
}

// §2.12.4 / §6.4.1 unit (G15): every native-binding-shaped crate in the core closure is CLASSIFIED, and the
// table is bijective with the live set — a new `-sys` crate reaching the core (a Dependabot bump) fails until
// its link class is stated here; a stale row fails until removed. The classification is the conscious-link
// ratchet on top of the stem deny-lists (a name heuristic — the deny-lists, not this table, are the
// forbidden set).
#[test]
fn every_native_binding_in_the_core_closure_is_classified() {
    let graph = LockGraph::parse(WORKSPACE_LOCK);
    let closure = graph.closure_of(CORE_CRATE);
    let shaped: BTreeSet<&str> = names_of(&closure)
        .into_iter()
        .filter(|n| is_native_binding_shaped(n))
        .collect();
    let classified: BTreeSet<&str> = CLASSIFIED_NATIVE_BINDINGS.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        classified.len(),
        CLASSIFIED_NATIVE_BINDINGS.len(),
        "§2.12.4: CLASSIFIED_NATIVE_BINDINGS carries a duplicate row"
    );
    let unclassified: Vec<&&str> = shaped.iter().filter(|n| !classified.contains(*n)).collect();
    assert!(
        unclassified.is_empty(),
        "§2.12.4: native-binding crate(s) reached the core closure without a link classification — state \
         WHY each is linked into the MIT core (HostOs / WebViewHost / MobileHost / WasmHost / PureRust) or \
         remove the edge: {unclassified:?}"
    );
    let stale: Vec<&&str> = classified.iter().filter(|n| !shaped.contains(*n)).collect();
    assert!(
        stale.is_empty(),
        "§2.12.4: classified binding(s) no longer in the core closure — remove the stale row(s): {stale:?}"
    );
    // the stem deny-lists remain the forbidden set — no classified crate may match one (a classification
    // never overrides a ban)
    let banned: Vec<String> = forbidden_in(&classified, G53_FORBIDDEN_STEMS)
        .into_iter()
        .chain(forbidden_in(&classified, SUBPROCESS_ONLY_DECODER_STEMS))
        .collect();
    assert!(
        banned.is_empty(),
        "§2.12.4: a classified binding matches a forbidden decoder stem — classification cannot admit a \
         banned decoder: {banned:?}"
    );
    // the LINK CLASS is load-bearing, not decorative: a `MobileHost` / `WasmHost` binding exists in the
    // closure only as a cfg'd-out transitive of the Tauri host — it must never be a DIRECT dependency of
    // the desktop core (a direct Android/wasm binding would be a real §1 desktop-only defect)
    let core_key = graph
        .keys_named(CORE_CRATE)
        .into_iter()
        .next()
        .expect("§2.12.4: the core crate is locked");
    let direct: BTreeSet<&str> = graph
        .edges
        .get(core_key)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();
    let direct_non_desktop: Vec<&&str> = CLASSIFIED_NATIVE_BINDINGS
        .iter()
        .filter(|(_, class)| matches!(class, LinkClass::MobileHost | LinkClass::WasmHost))
        .map(|(n, _)| n)
        .filter(|n| direct.contains(*n))
        .collect();
    assert!(
        direct_non_desktop.is_empty(),
        "§1/§2.12.4: a mobile/wasm host binding is a DIRECT `{CORE_CRATE}` dependency: {direct_non_desktop:?}"
    );
    assert!(
        direct.contains("windows-sys") && direct.contains("libc"),
        "§2.12.3/§2.1.2: the direct-dependency edge set was read (non-vacuity): {direct:?}"
    );
}

// ─── leg 2: the engine registry ──

/// The §3.2.2 / §2.12.4 kind of every [`EngineId`] — EXHAUSTIVE (the crate-root `wildcard_enum_match_arm`
/// deny forbids a catch-all), so a new engine id fails to compile until it is classified here. Every
/// third-party engine — FFmpeg / FFprobe / LibreOffice / poppler / pandoc / the ImageMagick delegate and the
/// libvips image-worker — is a `Subprocess`; ONLY ConvertIA's own MIT native CSV/TSV engine (§3.5.6) is
/// `InProcessNative`.
fn expected_kind(id: EngineId) -> EngineKind {
    match id {
        EngineId::FFmpeg
        | EngineId::FFprobe
        | EngineId::LibreOffice
        | EngineId::Poppler
        | EngineId::Pandoc
        | EngineId::ImageMagick
        | EngineId::ImageCore => EngineKind::Subprocess,
        EngineId::NativeCsvTsv => EngineKind::InProcessNative,
    }
}

// §2.12.4 / §3.2.2 / §6.4.1 unit (G15): over the LIVE §3.2.3 registry, every registered engine's descriptor
// kind matches its exhaustive classification, the descriptor's id is its own, and the native CSV/TSV engine
// is the ONE `InProcessNative` engine (registered — non-vacuous). The P5–P7 adapters are checked the moment
// they register: an image-worker adapter declaring `InProcessNative` reddens here.
#[test]
fn only_the_native_csv_tsv_engine_runs_in_core_every_other_engine_is_a_subprocess() {
    let registry = engine_registry()
        .expect("§3.2.3: the registered v1 set builds a valid single-owner registry");
    let mut in_core = Vec::new();
    for id in registry.serialised_flags().keys().copied() {
        let descriptor = registry
            .engine(id)
            .expect("§3.2.3: every flag-map id is a registered engine")
            .descriptor();
        assert_eq!(
            descriptor.id, id,
            "§3.2.2: an engine's descriptor carries its own id"
        );
        assert_eq!(
            descriptor.kind,
            expected_kind(id),
            "§2.12.4/§3.2.2: {id:?} must run as {:?}",
            expected_kind(id)
        );
        if descriptor.kind == EngineKind::InProcessNative {
            in_core.push(id);
        }
    }
    assert_eq!(
        in_core,
        vec![EngineId::NativeCsvTsv],
        "§2.12.4/§3.5.6: the native CSV/TSV engine is the ONE in-core engine, and it is registered"
    );
}

// ─── leg 3: the detection source ──

// §2.12.4 / §1.2 / §6.4.1 unit (G15): `crate::detection` — the first code touching untrusted bytes — reaches
// ONLY the vetted pure-Rust sniff crates, and carries no `unsafe`, no `extern`, no `#[link]`, no process or
// network path. A full-decoder crate (even a pure-Rust one) or a C binding imported here reddens: the
// absolute forbids both a C/C++ decoder AND a full decode in-core.
#[test]
fn detection_reaches_only_the_vetted_pure_rust_sniff_crates() {
    let sources: Vec<(PathBuf, String)> = production_sources(Path::new(CORE_SRC))
        .into_iter()
        .filter(|(rel, _)| under(rel, DETECTION_SUBTREE))
        .collect();
    assert!(
        !sources.is_empty(),
        "§1.2: the detection module has production source to scan"
    );
    let mut reached: BTreeSet<String> = BTreeSet::new();
    for (rel, code) in &sources {
        for needle in ["unsafe", "extern"] {
            assert!(
                !has_token(code, needle),
                "§2.12.4: {} carries a `{needle}` token — the in-core untrusted-byte module is pure safe Rust",
                rel.display()
            );
        }
        for needle in [
            concat!("#[", "link"),
            concat!("std::", "process"),
            concat!("std::", "net"),
            concat!("tokio::", "process"),
            concat!("tokio::", "net"),
            concat!("process::", "Command"),
        ] {
            assert!(
                !code.contains(needle),
                "§2.12.4: {} reaches `{needle}` — detection sniffs bytes, it never links, spawns or connects",
                rel.display()
            );
        }
        reached.extend(external_roots(code));
    }
    assert!(
        reached.contains("encoding_rs") && reached.contains("chardetng"),
        "§1.2 (P3.27): the text-encoding heuristic's crates are reached (scan non-vacuity): {reached:?}"
    );
    let unvetted: Vec<&String> = reached
        .iter()
        .filter(|r| !VETTED_DETECTION_CRATES.contains(&r.as_str()))
        .collect();
    assert!(
        unvetted.is_empty(),
        "§2.12.4: detection reaches crate root(s) outside the vetted pure-Rust sniff set — a full decoder or \
         a C binding in the trust kernel? Vet it against §2.12.4/§0.8 (with its §0.8 row) or remove it: \
         {unvetted:?}"
    );
}

// §6.4.1 unit (G15): the source model is exact where it matters — the stripper blanks strings / raw
// strings / char literals / both comment forms and keeps code (so a token in a comment or message never
// counts and a real token is never hidden), the token test honours identifier boundaries, and the root
// extractor binds `use` names, aliases, `self`-imports and local mods so only genuine path starts surface.
#[test]
fn the_source_model_strips_noncode_and_extracts_path_roots_exactly() {
    let src = concat!(
        "use std::io::{self, Read};\nuse enc_rs::{Encoding as Enc, UTF_8};\nmod inner;\n",
        "// uns", "afe in a comment\n/* ext", "ern in /* a nested */ block */\n",
        "let s = \"uns", "afe \\\" ext", "ern\"; let r = r#\"uns", "afe\"#; let c = '\"'; let l: &'a str;\n",
        "let d = chardetng_x::Detector::new(inner::thing(), io::stdin(), str::len(\"x\"), u8::MAX, Enc::y());\n",
        "let e = <T as Trait>::f(); let z = std::mem::take(&mut v); let g = Vec::<u8>::new();\n",
        "let img = ::image_x::io::Reader::open(\"x\"); let n = (a)::b();\n"
    );
    let code = strip_noncode(src);
    for hidden in ["uns\u{61}fe", "ext\u{65}rn", "nested"] {
        assert!(
            !has_token(&code, hidden),
            "`{hidden}` must be blanked with its literal/comment"
        );
    }
    assert!(
        has_token(&code, "chardetng_x") && has_token(&code, "take"),
        "code survives"
    );
    assert!(
        !has_token(&code, "chardetng"),
        "identifier boundaries: `chardetng_x` is not `chardetng`"
    );
    assert!(
        code.contains("&'a str"),
        "a lifetime tick is not a char literal: {code}"
    );
    // a backslash-NEWLINE string continuation keeps its newline, so the projection stays line-aligned with
    // the source (the round-3 review P3)
    let continued = "let m = \"first \\\n     second\";\nfn after() {}\n";
    assert_eq!(
        strip_noncode(continued).lines().count(),
        continued.lines().count(),
        "a backslash-newline continuation must not swallow its line"
    );
    let roots = external_roots(&code);
    let expected: BTreeSet<String> = ["enc_rs", "chardetng_x", "image_x"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        roots, expected,
        "only genuine external path starts surface — incl. the head of a global `::image_x::…` path, \
         never a `<T as Trait>::f` / `Vec::<u8>::new` / `(a)::b` continuation: {code}"
    );
}

// §6.4.1 unit (G15): the leg-4 predicate is ARMED — each violation shape fires on a planted positive over
// the same strip-then-project pipeline the real scan uses, and the must-not-fire shapes stay silent: (1) an
// `unsafe` block in production code is found even when a doc comment ABOVE it mentions the `#[cfg(test)]`
// marker (the phantom-gate blinding the round-1 review caught); (2) the REAL attribute still gates, so an
// `unsafe` inside the test module is NOT reported; (3)–(5) the multi-item / spaced / inner-attribute
// `allow` + `expect` spellings all fire; (6) the token inside a string is silent; (7) the allow-listed
// subtree is exempt; (8)–(12) the projection keeps scanning PAST a gated item — a statement-level seam, a
// mid-file test module, an `all(test, …)` gate vs a production `not(test)`/`any(test, …)` (in either
// spacing, at any nesting), a gated `;`-declaration — and a whole-file `#![cfg(test)]` is empty. Leg (13)
// pins a DIFFERENT predicate that the same real-tree test depends on: the crate-root check's
// inner-attribute anchor. A scanner that fails any of these is fail-OPEN, which is exactly the shape a
// file-count guard cannot see.
#[test]
fn the_unsafe_policy_predicate_fires_on_every_planted_shape() {
    let marker = concat!("#[cfg", "(test)]");
    let uns = "uns\u{61}fe";
    let unsafe_code = "uns\u{61}fe_code";
    let rel = Path::new("ipc/mod.rs");
    let scan =
        |src: &str| unsafe_policy_violations(rel, &production_projection(&strip_noncode(src)));

    let truncation_bait = format!("//! docs mention {marker} here\nfn a() {{ {uns} {{ }} }}\n{marker}\nmod t {{ fn b() {{ {uns} {{}} }} }}\n");
    let found = scan(&truncation_bait);
    assert_eq!(
        found.len(),
        1,
        "(1)+(2): the production `{uns}` fires once; the test module's is blanked: {found:?}"
    );

    for allow in [
        format!("#[allow({unsafe_code}, dead_code)]"),
        format!("#![allow( {unsafe_code} )]"),
        format!("#[allow(dead_code, {unsafe_code})]"),
        format!("#[expect({unsafe_code})]"),
    ] {
        let src = format!("{allow}\nfn a() {{}}\n");
        assert_eq!(scan(&src).len(), 1, "(3)–(5): `{allow}` must fire");
    }

    let quiet = format!("fn a() {{ let s = \"{uns} {unsafe_code}\"; }} // {uns}\n");
    assert!(
        scan(&quiet).is_empty(),
        "(6): a token inside a string/comment is silent"
    );

    let exempt = format!("fn a() {{ {uns} {{ }} }}\n#[allow({unsafe_code})]\nmod ffi {{}}\n");
    let exempt_rel = Path::new(ALLOWED_UNSAFE_SUBTREE).join("mod.rs");
    assert!(
        unsafe_policy_violations(&exempt_rel, &production_projection(&strip_noncode(&exempt)))
            .is_empty(),
        "(7): the allow-listed subtree is exempt"
    );
    assert_eq!(
        scan(&exempt).len(),
        2,
        "…and the same source outside it fires both shapes"
    );

    // (8) a STATEMENT-LEVEL seam inside a production fn (the fs_guard P3.65 shape) skips only its own
    // block: the `{uns}` inside the seam is silent, the `{uns}` AFTER it in the same fn fires — the
    // round-2 review P1 (a first-marker prefix cut hid everything after the seam).
    let seam = format!(
        "fn a(x: [u8; 4]) {{\n    {marker}\n    if armed() {{ {uns} {{ }} }}\n    {uns} {{ }}\n}}\n"
    );
    let found = scan(&seam);
    assert_eq!(
        found.len(),
        1,
        "(8): the seam is skipped, the production `{uns}` after it fires: {found:?}"
    );
    // (9) a gated test MODULE mid-file is skipped whole; a production fn AFTER it is still scanned
    let after_module = format!(
        "{marker}\nmod tests {{ fn t() {{ {uns} {{}} }} }}\nfn later() {{ {uns} {{ }} }}\n"
    );
    assert_eq!(
        scan(&after_module).len(),
        1,
        "(9): the production fn after a gated test module fires"
    );
    // (10) `all(test, …)` gates too; `not(test)` is production
    let all_gate = format!("#[cfg(all(test, unix))]\nfn t() {{ {uns} {{}} }}\n");
    assert!(
        scan(&all_gate).is_empty(),
        "(10a): an `all(test, …)` conjunction is a test gate"
    );
    let not_gate = format!("#[cfg(not(test))]\nfn p() {{ {uns} {{}} }}\n");
    assert_eq!(scan(&not_gate).len(), 1, "(10b): `not(test)` is production");
    // (10c) the exclusion is SCOPE-aware, not a substring: a spaced or nested negation, and a `test`
    // reached through a DISJUNCTION at any depth (`all(any(test, unix), windows)` compiles on unix in a
    // production build), are production and must not be blanked (the round-3 review P2 + the round-4
    // review P2 — the fail-OPEN direction)
    for production_only in [
        format!("#[cfg(all(not( test ), unix))]\nfn p() {{ {uns} {{}} }}\n"),
        format!("#[cfg(all(not(all(test)), unix))]\nfn p() {{ {uns} {{}} }}\n"),
        format!("#[cfg(all(any(test, debug_assertions), unix))]\nfn p() {{ {uns} {{}} }}\n"),
        format!("#[cfg(all(all(any(test, unix)), windows))]\nfn p() {{ {uns} {{}} }}\n"),
    ] {
        assert_eq!(
            scan(&production_only).len(),
            1,
            "(10c): a `test` reached through a negation or a disjunction — at any spacing or nesting — is \
             production, never a test gate: {production_only}"
        );
    }
    // (11) a gate followed by further outer attributes + a `;`-terminated declaration
    let decl = format!("{marker}\n#[derive(Debug)]\nstruct T;\nfn p() {{ {uns} {{}} }}\n");
    assert_eq!(
        scan(&decl).len(),
        1,
        "(11): the gated `;`-declaration is skipped, the fn after it fires"
    );
    // (12) a whole-file `#![cfg(test)]` inner attribute yields an empty projection
    let whole = format!("#![cfg(test)]\nfn t() {{ {uns} {{}} }}\n");
    assert!(
        scan(&whole).is_empty(),
        "(12): a `#![cfg(test)]` file is test-only"
    );

    // (13) the crate-root deny check is ANCHORED on the inner-attribute form: `#![deny(…)]` / a spaced
    // `#! [ forbid ( … ) ]` satisfy it, while an item-level `#[deny(…)]` or a `cfg_attr`-wrapped deny —
    // both of which G29's own `_DENY_RE` rejects — do NOT (the round-5 review finding: the round-4
    // deny-OR-forbid widening had dropped the anchor the exact-string check carried)
    let root_denies = |src: &str| {
        let code = production_projection(&strip_noncode(src));
        has_inner_lint_attr(&code, "deny", unsafe_code)
            || has_inner_lint_attr(&code, "forbid", unsafe_code)
    };
    for real_root in [
        format!("#![deny({unsafe_code})]\nfn a() {{}}\n"),
        format!("#! [ forbid ( {unsafe_code} , dead_code ) ]\nfn a() {{}}\n"),
    ] {
        assert!(
            root_denies(&real_root),
            "(13): `{real_root}` IS a crate-root deny"
        );
    }
    for not_a_root in [
        format!("#[deny({unsafe_code})]\nmod m {{}}\n"),
        format!("#![cfg_attr(feature = \"x\", deny({unsafe_code}))]\nfn a() {{}}\n"),
        "#![deny(clippy::all)]\nfn a() {}\n".to_owned(),
    ] {
        assert!(
            !root_denies(&not_a_root),
            "(13): `{not_a_root}` is NOT a crate-root `{unsafe_code}` deny"
        );
    }
}

// §6.4.1 unit (G15): the test-only-file resolution is bound to the declaring DIRECTORY and located over
// stripped text — the real tree resolves this module, `fuzz_replay` and `orchestrator`'s e2e file (non-
// vacuity), never a production file (`detection/mod.rs`).
#[test]
fn test_only_file_modules_resolve_to_their_declaring_directory() {
    let test_only = test_only_file_modules(Path::new(CORE_SRC));
    for present in [
        "untrusted_byte_boundary.rs",
        "fuzz_replay.rs",
        "orchestrator/cross_volume_e2e_tests.rs",
    ] {
        assert!(
            test_only.contains(Path::new(present)),
            "§0.7: `{present}` is a `#[cfg(test)]`-declared file module (found: {test_only:?})"
        );
    }
    assert!(
        !test_only.contains(Path::new("detection/mod.rs")),
        "§1.2: the detection module is production source, never skipped"
    );
}

// ─── leg 4: the whole-core unsafe policy (the G29 mirror) ──

// §2.12.4 / G29 / §6.4.1 unit (G15): both crate roots deny `unsafe_code`; no production source outside the
// single allow-listed FFI subtree `src/platform/**` carries an `unsafe` token or an `allow(unsafe_code)`;
// and the allow-listed subtree is the only place an allow appears. The cargo-test-plane companion of the
// G29 unsafe-policy gate, so the in-core untrusted-byte path stays safe Rust on every `cargo test`.
#[test]
fn no_unsafe_outside_the_allow_listed_platform_module() {
    let root = Path::new(CORE_SRC);
    let sources = production_sources(root);
    for crate_root in CRATE_ROOTS {
        let (_, code) = sources
            .iter()
            .find(|(rel, _)| rel == Path::new(crate_root))
            .expect("§0.7: both crate roots exist");
        // `deny` OR `forbid`, any spelling — the same pair G29's own `_DENY_RE` accepts, so tightening a
        // root to `forbid` does not red this companion (the round-4 review P3) — but ANCHORED on the
        // crate-INNER `#![…]` form G29 also requires, so an item-level deny elsewhere in the file cannot
        // stand in for the crate root (the round-5 review finding)
        assert!(
            has_inner_lint_attr(code, "deny", "unsafe_code")
                || has_inner_lint_attr(code, "forbid", "unsafe_code"),
            "G29: `{crate_root}` must deny (or forbid) the `unsafe_code` lint at the CRATE ROOT (`#![…]`)"
        );
    }
    let violations: Vec<String> = sources
        .iter()
        .flat_map(|(rel, code)| unsafe_policy_violations(rel, code))
        .collect();
    assert!(
        violations.is_empty(),
        "G29/§2.12.4: unsafe-policy violation(s) in production source: {violations:?}"
    );
    // Non-vacuity, CONTENT-based (a file count cannot see a blinded scan): the allow-listed subtree's real
    // `unsafe` + its `allow(unsafe_code)` are SEEN by the same strip-then-project pipeline (the one place
    // both legitimately exist); the scan reaches the bulk of the production tree (a NON-BLANK line floor
    // close under the measured 8 132 — comment lines strip to blank and are not counted — so blinding even
    // one large module reddens); and each historically-blinded file carries its OWN content pin, naming a
    // production item that sits BEHIND the marker/seam that once hid it.
    let platform_code = sources
        .iter()
        .filter(|(rel, _)| under(rel, ALLOWED_UNSAFE_SUBTREE))
        .map(|(_, code)| code.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        has_token(&platform_code, "unsafe") && has_allow_unsafe(&platform_code),
        "G29: the allow-listed `src/{ALLOWED_UNSAFE_SUBTREE}/` FFI module carries the crate's `unsafe` + its \
         `allow(unsafe_code)` — the scan must SEE them (a blind scanner passes vacuously)"
    );
    let production_lines: usize = sources
        .iter()
        .map(|(_, code)| code.lines().filter(|l| !l.trim().is_empty()).count())
        .sum();
    assert!(
        // deliberately tight (4 % under the live 8 132): the floor is a RATCHET, so a red here is read as
        // "what stopped being scanned?" first. Lowering it is a test relaxation under test-strategy §8 and
        // needs the `[Test-Change]` justification, never a reflexive nudge (the round-4 review P3).
        production_lines > 7_800,
        "§0.7: the production tree was walked in full ({production_lines} non-blank stripped production lines)"
    );
    // Per-file content pins — one per file a review round caught being blinded, each naming a production
    // item BEHIND its marker/seam plus (for the seam case) a test-only item that must NOT survive. A line
    // floor alone is too coarse: blinding `fs_guard` + `lib.rs` + `pool` cost ~1 000 of the 8 132 lines and
    // would still have cleared any round number (the round-3 review P3).
    let file_of = |name: &str| -> &str {
        sources
            .iter()
            .find(|(rel, _)| rel == Path::new(name))
            .map(|(_, code)| code.as_str())
            .expect("§0.7: the pinned module exists")
    };
    // `fs_guard/mod.rs`: a `#[cfg(test)]` fault-injection seam INSIDE a production fn sits ahead of the
    // whole §2.0 publish/plan kernel — the projection reaches `compute_output_plan` (production, after the
    // seam) while the seam's own test-only `fat_class_destination` module never appears (round-2 P1).
    let fs_guard = file_of("fs_guard/mod.rs");
    assert!(
        has_token(fs_guard, "compute_output_plan") && !has_token(fs_guard, "fat_class_destination"),
        "§2.0: the projection reaches the production kernel AFTER fs_guard's statement-level test seam and \
         blanks the seam's test-only module (a first-marker prefix cut would fail both halves)"
    );
    // `lib.rs`: a gated `mod tests` nested inside `launch_intake` precedes the §7.x boot spine (round-2 P1).
    assert!(
        has_token(file_of("lib.rs"), "generate_context"),
        "§7.2: the projection reaches `run()`'s Builder chain AFTER lib.rs's nested gated test module"
    );
    // `pool/mod.rs`: the gated test-only `fn close` precedes the §0.9 degree formula (round-2 P1).
    assert!(
        has_token(file_of("pool/mod.rs"), "clamp_global_degree"),
        "§0.9: the projection reaches the degree formula AFTER pool's gated test-only seam"
    );
    // `ipc/mod.rs`: its `//!` header MENTIONS the marker in prose (round-1 P1) — the projection must reach
    // the declarations below it. Pinned on a real production token, never on a line count: blanking keeps
    // every newline, so a fully-blinded file still reports its full length (the round-3 review P3).
    assert!(
        has_token(file_of("ipc/mod.rs"), "APP_CLOSE_REQUESTED"),
        "§0.4: `ipc/mod.rs`'s production projection reaches its declarations — its `//!` header mentions the \
         test marker and must not blind the scan (the round-1 review P1)"
    );
}
