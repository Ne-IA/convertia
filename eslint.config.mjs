// eslint.config.mjs -- the section 5.1 flat ESLint config (the G5 lint plane + the G57
// English-only legs). It carries every project-local rule the gates freeze onto this config:
//   check-ts-gate    -> @typescript-eslint/no-explicit-any + the fc.gen restriction.
//   check-english-only (G57) -> react/jsx-no-literals (no inline UI text; strings live in
//                        src/strings/ui.ts, section 5.7) + a no-restricted-imports ban on the
//                        i18n-runtime libraries (English-only, no i18n runtime; section 6.10).
//
// ESM (.mjs) on purpose: package.json declares no type=module, so a .js flat config with an
// import statement would fail to load under the Node CommonJS default. eslint is pinned to the
// 9.x line (not the 10.x latest): eslint-plugin-react 7.37 -- required for react/jsx-no-literals
// -- peers only up to eslint 9.7, so 9.x is the ecosystem-compatible choice (section 5.1
// avoid-bleeding-edge-that-drifts). The single-IPC-consumer boundary rule (only src/lib/ipc
// imports the Tauri API) is the separate P1.36 box, not this one.
// [Build-Session-Entscheidung: P1.33]
import tseslint from "typescript-eslint";
import globals from "globals";
import react from "eslint-plugin-react";

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
      // Belt-and-suspenders to the check-english-only dependency + import scan.
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "i18next",
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
            {
              name: "react-i18next",
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
            {
              name: "react-intl",
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
            {
              name: "next-intl",
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
            {
              name: "vue-i18n",
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
          ],
          patterns: [
            {
              group: ["@lingui/*", "@formatjs/*", "i18next-*"],
              message: "English-only: no i18n runtime ships (section 5.7 / 6.10).",
            },
          ],
        },
      ],
    },
  },
);
