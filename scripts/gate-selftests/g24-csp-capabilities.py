#!/usr/bin/env python3
"""g24-csp-capabilities.py - G24 self-test for check-csp-capabilities (P0.3.2, G47).

Proves the CSP/capability lint CATCHES every §0.10 widening (a remote CSP origin, a missing/extra
directive, a flipped dangerous key, a re-enabled updater, a remote window url, devtools, ANY capability
grant beyond core/log/store — incl. shell:allow-spawn/default, fs:/opener:/dialog:/http:/updater:, a
`remote`/`urls` grant, an INLINE conf capability — a missing/wrong dns-prefetch meta, a custom URL
scheme) and PASSES the locked posture — plus the live main() over a temp fixture tree. stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-csp-capabilities"
_loader = importlib.machinery.SourceFileLoader("ccc", str(SCRIPT))
_spec = importlib.util.spec_from_loader("ccc", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def valid_conf() -> dict:
    return {
        "app": {
            "withGlobalTauri": False,
            "windows": [{"url": "index.html"}],
            "security": {"csp": dict(m.LOCKED_CSP), "assetProtocol": {"enable": False}},
        },
        "bundle": {"createUpdaterArtifacts": False},
    }


# --- pure evaluate_conf -----------------------------------------------------------------------
record("locked conf -> no problems", m.evaluate_conf(valid_conf()) == [])

c = valid_conf(); c["app"]["security"]["csp"]["connect-src"] = "'self' https://evil.example"
record("CSP connect-src widened to a remote origin -> caught", m.evaluate_conf(c) != [])

c = valid_conf(); del c["app"]["security"]["csp"]["frame-ancestors"]
record("CSP missing a locked directive -> caught", m.evaluate_conf(c) != [])

c = valid_conf(); c["app"]["security"]["csp"]["child-src"] = "*"
record("CSP has an UNEXPECTED directive -> caught", any("UNEXPECTED" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["withGlobalTauri"] = True
record("withGlobalTauri=true -> caught", any("withGlobalTauri" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["security"]["dangerousDisableAssetCspModification"] = ["script-src"]
record("dangerousDisableAssetCspModification set -> caught",
       any("dangerousDisableAssetCspModification" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["security"]["dangerousRemoteDomainIpcAccess"] = [{"domain": "evil.example"}]
record("dangerousRemoteDomainIpcAccess set -> caught",
       any("dangerousRemoteDomainIpcAccess" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["security"]["assetProtocol"]["enable"] = True
record("assetProtocol.enable=true -> caught", any("assetProtocol" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["bundle"]["createUpdaterArtifacts"] = True
record("bundle.createUpdaterArtifacts=true -> caught",
       any("createUpdaterArtifacts" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["bundle"]["updater"] = {"pubkey": "x"}
record("bundle.updater set -> caught", any("bundle.updater" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["plugins"] = {"updater": {"endpoints": ["https://x"]}}
record("plugins.updater present -> caught", any("plugins.updater" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["plugins"] = {"deep-link": {"desktop": {"schemes": ["convertia"]}}}
record("plugins.deep-link present -> caught", any("deep-link" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["windows"][0]["url"] = "https://app.example"
record("window.url remote -> caught", any("windows[].url" in p for p in m.evaluate_conf(c)))

c = valid_conf(); c["app"]["windows"][0]["devtools"] = True
record("window.devtools=true -> caught", any("devtools" in p for p in m.evaluate_conf(c)))

# INLINE capability in the conf (the path the on-disk scan never traverses)
c = valid_conf()
c["app"]["security"]["capabilities"] = [{"identifier": "evil", "windows": ["*"],
                                         "remote": {"urls": ["https://evil"]}, "permissions": ["fs:default"]}]
record("INLINE app.security.capabilities[] with fs:/remote -> caught", m.evaluate_conf(c) != [])
c = valid_conf()
c["app"]["security"]["capabilities"] = ["main"]   # a STRING entry is a file ref (dir-scan covers it) -> OK
record("INLINE capability STRING ref -> not flagged (file-scan covers it)", m.evaluate_conf(c) == [])

# --- pure evaluate_capability (ALLOW-list: only core/log/store) -------------------------------
record("clean capability (core/log/store) -> no problems",
       m.evaluate_capability({"permissions": ["core:default", "log:default", "store:allow-get"]}) == [])
for perm in ("fs:allow-read", "opener:allow-open-url", "dialog:allow-open", "http:default",
             "updater:allow-check", "shell:allow-execute", "shell:allow-spawn", "shell:default",
             "shell:allow-sidecar", "geo:default"):
    record(f"capability grant `{perm}` -> caught (allow-list)",
           m.evaluate_capability({"permissions": [perm]}, "main.json") != [])
record("object-form fs: permission -> caught",
       m.evaluate_capability({"permissions": [{"identifier": "fs:allow-read"}]}) != [])
record("capability with a `remote.urls` grant -> caught",
       m.evaluate_capability({"permissions": ["core:default"], "remote": {"urls": ["https://evil"]}}) != [])
record("capability with a top-level `urls` grant -> caught",
       m.evaluate_capability({"permissions": ["core:default"], "urls": ["https://evil"]}) != [])
record("malformed `permissions` (a dict, not a list) -> caught (fail-closed)",
       m.evaluate_capability({"permissions": {"fs:allow-read": True}}) != [])

# --- pure evaluate_index_html (meta matched as a unit, either order) --------------------------
record("index.html WITH dns-prefetch meta -> ok",
       m.evaluate_index_html('<meta http-equiv="x-dns-prefetch-control" content="off">') == [])
record("index.html WITH meta in reversed attr order -> ok",
       m.evaluate_index_html('<meta content="off" http-equiv="x-dns-prefetch-control" />') == [])
record("index.html WITHOUT the meta -> caught",
       m.evaluate_index_html("<html><head></head></html>") != [])
record("index.html with content=\"on\" -> caught (wrong value)",
       m.evaluate_index_html('<meta http-equiv="x-dns-prefetch-control" content="on">') != [])
record("index.html with non-collocated tokens -> caught (not one meta element)",
       m.evaluate_index_html('<!-- x-dns-prefetch-control --><meta name="z" content="off">') != [])

# --- live main() over a temp fixture tree -----------------------------------------------------
def write_tree(td: Path, conf: dict, perms: list, index: str) -> None:
    (td / "tauri.conf.json").write_text(json.dumps(conf), encoding="utf-8")
    (td / "capabilities").mkdir(exist_ok=True)
    (td / "capabilities" / "main.json").write_text(json.dumps({"permissions": perms}), encoding="utf-8")
    (td / "index.html").write_text(index, encoding="utf-8")


META = '<meta http-equiv="x-dns-prefetch-control" content="off">'
with tempfile.TemporaryDirectory() as d:
    td = Path(d)
    argv = ["--conf", str(td / "tauri.conf.json"), "--capabilities", str(td / "capabilities"),
            "--index", str(td / "index.html"), "--src-tauri", str(td)]

    write_tree(td, valid_conf(), ["core:default", "log:default"], META)
    record("main(): valid fixture tree -> exit 0", m.main(argv) == 0)

    write_tree(td, valid_conf(), ["core:default", "fs:allow-read"], META)
    record("main(): an fs: capability grant -> exit 1", m.main(argv) == 1)

    # a custom-URL-scheme registration file under --src-tauri -> exit 1 (exercises the scheme path +
    # the out-of-ROOT relative_to fallback, since td is outside the repo root)
    write_tree(td, valid_conf(), ["core:default"], META)
    (td / "Info.plist").write_text("<plist><dict><key>CFBundleURLTypes</key></dict></plist>", encoding="utf-8")
    record("main(): a CFBundleURLTypes plist under src-tauri -> exit 1 (no crash)", m.main(argv) == 1)
    (td / "Info.plist").unlink()

    (td / "tauri.conf.json").write_text("{ not json", encoding="utf-8")
    record("main(): unparseable conf -> exit 2", m.main(argv) == 2)

record("main(): target absent -> skip exit 0", m.main(["--conf", str(Path(d) / "gone.json")]) == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-csp-capabilities] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
