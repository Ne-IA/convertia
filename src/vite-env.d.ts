/// <reference types="vite/client" />

// Vite ambient types for the WebView build (§0.4.0 / §5.1). Provides the module declarations for
// Vite's asset-import graph — most importantly the CSS side-effect import `import "./styles/app.css"`
// in main.tsx (P1.32 Tailwind entry), which `tsc --noEmit` would otherwise reject as an
// unresolved module. Reference-only; it declares no values and ships nothing.
