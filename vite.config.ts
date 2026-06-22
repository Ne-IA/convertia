import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";

// ConvertIA frontend build config (§5.1) — the Vite bundle the Tauri WebView loads.
//
// [Build-Session-Entscheidung: P1.29] build.target = the §0.3.1 WebView INTERSECTION floor.
// The supported-OS floor's binding constraint is macOS 11 Big Sur (WKWebView ~= Safari 14);
// Win10-1809+ WebView2 (Evergreen Chromium) and Linux WebKitGTK 4.1 both cover the Safari-14
// feature set, so a single `safari14` target is the §5.1 "target the intersection, avoid
// bleeding-edge CSS/JS that drifts" floor — simpler + deterministic than a per-platform split,
// and it needs no Node `process` reference. index.html + the React 19 mount arrive in
// P1.23/P1.31; this config is the build seam P1.16's empty-window frame and `tauri build` consume.
//
// [Build-Session-Entscheidung: P1.29] plugin = @vitejs/plugin-react-swc (not the Babel-based
// @vitejs/plugin-react) — its only required peer is `vite ^8` (SWC is self-contained, no Babel
// chain / optional rolldown-babel/react-compiler peers), the lighter, more predictable Vite-8
// choice per §5.1's lightweight principle.
export default defineConfig({
  plugins: [react()],
  // Tauri keeps its own console output visible — do not let Vite clear the screen.
  clearScreen: false,
  server: {
    // Must match tauri.conf.json `build.devUrl` (http://localhost:1420); strict so a port
    // collision fails loudly instead of silently moving Tauri's WebView off the dev server.
    port: 1420,
    strictPort: true,
  },
  build: {
    // §0.3.1 intersection floor (see the decision note above).
    target: "safari14",
    // tauri.conf.json `build.frontendDist` is "../dist" (relative to src-tauri/) = this repo-root dir.
    outDir: "dist",
  },
});
