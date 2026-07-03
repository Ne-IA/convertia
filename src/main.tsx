// src/main.tsx — the React 19 root mount (§5.1 / §0.4.0).
//
// The Vite entry index.html (P1.23) references this module via
// `<script type="module" src="/src/main.tsx">` and provides the `#root` mount target.
// This is the minimal bootable mount; the screen-state shell it renders is App.tsx (P1.31),
// and the §5.2 finite-state machine + per-state screens arrive in P3+.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
// The §5.5 Tailwind v4 entry + design tokens (P1.32); a global side-effect stylesheet import.
import "./styles/app.css";
// [Build-Session-Entscheidung: P2.95] §7.5.1 the frontend-error → log-file bridge, imported from the
// src/lib/ipc/** façade — NEVER @tauri-apps/plugin-log directly (the §5.1 one-IPC-consumer discipline).
import { installFrontendErrorLog } from "./lib/ipc/log";

// [Build-Session-Entscheidung: P2.95] Install the §7.5.1 error bridge as early as possible — before the
// mount — so a fault anywhere in the app (incl. the initial render) is recorded locally (§2.13), logging
// only the Error TYPE + source location, never the message (§7.5.3 / §0.11 T2b).
installFrontendErrorLog();

const rootElement = document.getElementById("root");
if (rootElement === null) {
  // The mount target is authored in index.html (P1.23); its absence is an unrecoverable
  // boot precondition failure, not a runtime state the UI recovers from.
  throw new Error("ConvertIA: #root mount target is missing from index.html");
}

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
