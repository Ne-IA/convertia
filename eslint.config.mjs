// eslint.config.mjs -- the section 5.1 flat ESLint config (the G5 lint plane + the G57
// English-only legs). It carries every project-local rule the gates freeze onto this config:
//   check-ts-gate    -> @typescript-eslint/no-explicit-any + the fc.gen restriction.
//   check-english-only (G57) -> react/jsx-no-literals (no inline UI text; strings live in
//                        src/strings/ui.ts, section 5.7) + a no-restricted-imports ban on the
//                        i18n-runtime libraries (English-only, no i18n runtime; section 6.10).
//   section 5.1 / G5 (P1.36) -> the single-IPC-consumer boundary: only src/lib/ipc/** may import the
//                        Tauri IPC surface (@tauri-apps/api or a @tauri-apps/plugin-* package); feature
//                        code talks to the typed facade.
//
// ESM (.mjs) on purpose: package.json declares no type=module, so a .js flat config with an
// import statement would fail to load under the Node CommonJS default. eslint is pinned to the
// 9.x line (not the 10.x latest): eslint-plugin-react 7.37 -- required for react/jsx-no-literals
// -- peers only up to eslint 9.7, so 9.x is the ecosystem-compatible choice (section 5.1
// avoid-bleeding-edge-that-drifts). [Build-Session-Entscheidung: P1.33, P1.36]
import tseslint from "typescript-eslint";
import globals from "globals";
import react from "eslint-plugin-react";

// The i18n-runtime / locale-switch import ban (G57 leg a). Single-homed here so the base block and
// the non-IPC override (P1.36) share one i18n list rather than duplicating it.
const I18N_RESTRICTED = {
  paths: [
    { name: "i18next", message: "English-only: no i18n runtime ships (section 5.7 / 6.10)." },
    { name: "react-i18next", message: "English-only: no i18n runtime ships (section 5.7 / 6.10)." },
    { name: "react-intl", message: "English-only: no i18n runtime ships (section 5.7 / 6.10)." },
    { name: "next-intl", message: "English-only: no i18n runtime ships (section 5.7 / 6.10)." },
    { name: "vue-i18n", message: "English-only: no i18n runtime ships (section 5.7 / 6.10)." },
  ],
  patterns: [
    {
      group: ["@lingui/*", "@formatjs/*", "i18next-*"],
      message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
    },
  ],
};

// The section 5.1 single-IPC-consumer boundary (P1.36, G5): only src/lib/ipc/** may import the Tauri
// IPC surface — @tauri-apps/api AND any @tauri-apps/plugin-* package (a plugin JS package is itself an
// IPC consumer: it wraps a `plugin:<name>|…` invoke channel, e.g. the @tauri-apps/plugin-log error() call
// issues `plugin:log|log`). Every other module talks to the typed facade (commands.ts / events.ts / log.ts), so
// the IPC contract has exactly one consumer and "no raw invoke in feature code" is lint-enforced. The
// plugin-* leg was added with the first plugin JS package, @tauri-apps/plugin-log (P2.95) — without it a
// direct `@tauri-apps/plugin-log` import outside the facade would slip the api-only ban (green-but-blind).
const TAURI_IPC_RESTRICTED_PATTERN = {
  group: ["@tauri-apps/api", "@tauri-apps/api/*", "@tauri-apps/plugin-*", "@tauri-apps/plugin-*/*"],
  message:
    "Only src/lib/ipc/** may import the Tauri IPC surface (@tauri-apps/api or a @tauri-apps/plugin-* package); feature code uses the typed facade (section 5.1).",
};

export default tseslint.config(
  // Not source: build output (Vite + Cargo), the Rust side, deps, the pinned gate tools, and .d.ts
  // shims (src/vite-env.d.ts carries a triple-slash reference the recommended set would flag).
  {
    ignores: [
      "dist/**",
      "target/**",
      "src-tauri/**",
      "node_modules/**",
      ".gate-tools/**",
      "**/*.d.ts",
    ],
  },
  ...tseslint.configs.recommended,
  {
    plugins: { react },
    languageOptions: { globals: { ...globals.browser } },
    settings: { react: { version: "detect" } },
    rules: {
      // check-ts-gate: the generated bindings.ts IPC door is fully typed, never the any type.
      "@typescript-eslint/no-explicit-any": "error",
      // check-ts-gate + G9 invariant (f): a bare fc.gen call defeats fast-check automatic shrinking,
      // so it is restricted to the approved shrink-wrapper (section 6.4.2).
      "no-restricted-syntax": [
        "error",
        {
          selector: "CallExpression[callee.object.name='fc'][callee.property.name='gen']",
          message:
            "fc.gen bypasses fast-check shrinking -- use the approved shrink-wrapper (section 6.4.2 / G9 invariant (f)).",
        },
      ],
      // G57 leg c: no inline user-facing literal in JSX; UI strings live in src/strings/ui.ts (5.7).
      "react/jsx-no-literals": "error",
      // G57 leg a: ban every i18n-runtime / locale-switch import (English-only, no i18n; section 6.10).
      "no-restricted-imports": ["error", I18N_RESTRICTED],
    },
  },
  {
    // section 5.1 / G5 (P1.36): the single-IPC-consumer boundary. Applies to all frontend source
    // EXCEPT src/lib/ipc/** (the sanctioned IPC door), overriding no-restricted-imports to ALSO ban the
    // Tauri IPC surface (@tauri-apps/api + @tauri-apps/plugin-*) there; src/lib/ipc/** keeps the base
    // i18n-only ban (it MAY import the Tauri API + plugin packages).
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/lib/ipc/**"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: I18N_RESTRICTED.paths,
          patterns: [...I18N_RESTRICTED.patterns, TAURI_IPC_RESTRICTED_PATTERN],
        },
      ],
    },
  },
);
