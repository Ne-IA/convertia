// src/main.tsx — the React 19 root mount (§5.1 / §0.4.0).
//
// The Vite entry index.html (P1.23) references this module via
// `<script type="module" src="/src/main.tsx">` and provides the `#root` mount target.
// This is the minimal bootable mount; the screen-state shell it renders is App.tsx (P1.31),
// and the §5.2 finite-state machine + per-state screens arrive in P3+.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";

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
