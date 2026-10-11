// eslint.config.mjs -- the section 5.1 flat ESLint config (the G5 lint plane + the G57
// English-only legs). It carries every project-local rule the gates freeze onto this config:
//   check-ts-gate    -> @typescript-eslint/no-explicit-any + the fc.gen restriction.
//   check-ts-gate (G5, security-concept section 5 T2) -> the T2 DOM/eval sink ban (T2_SINK_*
//                        below): each sink rule declared once at error, noInlineConfig on, every
//                        ban proven to fire on a planted fixture before each real run, and every
//                        src/ file linted under the evaluated sink rules, language options and
//                        settings the fixture proves.
//   check-english-only (G57) -> react/jsx-no-literals (no inline UI text; strings live in
//                        src/strings/ui.ts, section 5.7) + a no-restricted-imports ban on the
//                        i18n-runtime libraries (English-only, no i18n runtime; section 6.10).
//   section 5.1 / G5 (P1.36) -> the single-IPC-consumer boundary: only src/lib/ipc/** may import the
//                        Tauri IPC surface (@tauri-apps/api or a @tauri-apps/plugin-* package); feature
//                        code talks to the typed facade.
//
// ESM (.mjs) on purpose: package.json declares no type=module, so a .js flat config with an
// import statement would fail to load under the Node CommonJS default. eslint rides the 10.x line
// (since 2026-09-24; npm marks the whole 9.x line deprecated, "no longer supported"):
// eslint-plugin-react 7.37.5 -- required for react/jsx-no-literals -- declares no eslint-10 peer
// range (its list ends at ^9.7; pnpm reports the unmet peer as a warning) and the rules used here
// run unchanged on 10. Its React-version DETECTION would call the removed context.getFilename()
// under eslint 10, so settings.react.version is set explicitly below instead of "detect" -- read
// from the react pin in package.json, so a React bump never leaves it stale -- and a version-sensitive
// react/* rule that joins this config keeps that setting. (No backticks in these comments: the G57
// gate stashes backtick templates before it drops comments.) [Build-Session-Entscheidung: P1.33,
// P1.36; re-cut 2026-09-24 by the Co-Pilot dependency refresh]
import tseslint from "typescript-eslint";
import globals from "globals";
import react from "eslint-plugin-react";
import pkg from "./package.json" with { type: "json" };

// The React major.minor eslint-plugin-react keys its version-sensitive rules on, from the one pin.
const REACT_VERSION = pkg.dependencies.react.replace(/^[^0-9]*/, "");

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

// The T2 DOM/eval sink ban (security-concept section 5 T2; build-gates G5). WebView code may not
// name the DOM and eval sinks below, the APIs that parse a string as unsanitized markup or as code,
// in the forms this config reads, so no WebView-controlled value reaches one through those forms,
// whatever the flow depth; the G29 rule-(i) Semgrep taint rule is the defense in depth behind it. A
// property is banned wherever it is accessed as a property or destructured, on any receiver (read,
// write, call, optional chaining, a quoted or expression-free template name), so an aliased
// element, document or window fails as well. Each sink rule is declared exactly once, at error: a
// second, files-scoped declaration would replace its options for those files. noInlineConfig (base
// block) keeps an inline directive from lifting a ban. check-ts-gate freezes both; before every
// real run it lints the planted fixture scripts/gate-selftests/eslint-sink-fixture.tsx as
// src/main.tsx, failing unless each pinned sink reports its rule, and asks eslint for the evaluated
// config of every src/ source file, failing unless each carries the sink rules, language, language
// options and settings src/main.tsx does, with noInlineConfig on and no processor (so a files,
// ignores or basePath key on a block, or a nested config file, can neither narrow the ban nor
// change what its rules read).
// Residual, one class (measured): the ban matches each sink in the forms below, as the fixture
// pins them, and reads a set/createAttribute(NS) or createElement(NS) call and a global timer at
// the callee, directly or through exactly one wrapper node; every other indirection lints clean:
// such a call or timer reached as a template tag, through a callee wrapped more than once, through
// an alias or a destructured or imported binding of the method or timer, or through a comma
// expression, call, apply, bind or Reflect; a sink name held in a variable, built by
// concatenation, written after a template substitution or as the template key of an object
// literal, or passed as a string to Reflect or a property descriptor, and a name string wrapped in
// a type assertion; a javascript: URL whose scheme does not begin a string literal or the head of
// a template literal; a runtime string timer handler the type-aware rule does not check (one typed
// as a function by a type assertion or by a binding from an any value, one returned by a bind
// call, or one in a call whose own scope binds the timer name); and an API outside the list that
// none of the rules here reads (a JSX script element, the text of a script element, the
// constructor property of a function). At runtime the CSP script-src 'self' (no unsafe-eval or
// unsafe-inline source; G47) and the escaping React applies stay the compensating controls.
const T2_SINK_MESSAGE =
  "A T2 DOM/eval sink: WebView code never parses a string as HTML or as script (security-concept section 5 T2).";
// The sinks banned as properties on any receiver: the HTML-string sinks (Element innerHTML,
// outerHTML, insertAdjacentHTML and setHTMLUnsafe; Range createContextualFragment; Document
// parseHTMLUnsafe, write, writeln and execCommand, its insertHTML command; DOMParser
// parseFromString; the iframe srcdoc) and the code sinks eval and Function, so neither is named
// on a window alias (self, top, parent, frames, a defaultView or a variable).
const T2_SINK_PROPERTIES = [
  "innerHTML",
  "outerHTML",
  "insertAdjacentHTML",
  "setHTMLUnsafe",
  "createContextualFragment",
  "parseHTMLUnsafe",
  "write",
  "writeln",
  "execCommand",
  "parseFromString",
  "srcdoc",
  "eval",
  "Function",
].map((property) => ({ property, message: T2_SINK_MESSAGE }));
// The string-capable timers, banned as properties on every receiver except window and globalThis,
// the two receivers whose dotted or quoted timer member the type-aware no-implied-eval reads
// besides a bare call of the global name; the timer selectors below ban a global timer called
// through a callee neither handler rule reads.
const T2_TIMER_PROPERTIES = ["setTimeout", "setInterval"].map((property) => ({
  property,
  allowObjects: ["window", "globalThis"],
  message: T2_SINK_MESSAGE,
}));
// A call argument given as a string literal, or as a template literal whose head matches (an
// expression-free template is all head), so a name spelled with backticks is read as well.
const stringArgument = (index, pattern) =>
  `:matches([arguments.${index}.value=${pattern}], [arguments.${index}.quasis.0.value.cooked=${pattern}])`;
// One selector attribute per form in which the name of a member expression is read: a dotted
// name, a quoted name or the head of a template literal (an expression-free template is all head).
const propertyName = (member, pattern) =>
  ["name", "value", "quasis.0.value.cooked"].map(
    (form) => `[${member}.property.${form}=${pattern}]`,
  );
const anyOf = (attributes) => `:matches(${attributes.join(", ")})`;
// The callee one wrapper node holds as its expression: a parenthesized optional chain, a type
// assertion, satisfies, a non-null assertion or an instantiation expression. The fixture pins
// each but the angle-bracket type assertion, which only a .ts file can hold and which this path
// reads as it reads the as form. No selector reads deeper, so a callee wrapped twice is not read.
const WRAPPED_CALLEE = "callee.expression";
// A called method named by its member expression, optional chaining included, read on the callee
// itself or on the callee one wrapper node holds.
const memberName = (pattern) =>
  anyOf([...propertyName("callee", pattern), ...propertyName(WRAPPED_CALLEE, pattern)]);
// The global timers: setTimeout or setInterval called bare or as a window or globalThis member.
const TIMER = "/^set(?:Timeout|Interval)$/";
const TIMER_RECEIVER = "/^(?:window|globalThis)$/";
// A javascript: URL as a URL parser reads the string: leading control and space characters (U+0000
// to U+0020) are stripped, every tab and newline is removed, and the scheme matches in any letter
// case. The pattern is built from the bare scheme name, so this file holds no string that the
// pattern or no-script-url reads.
const URL_GAP = "[\\t\\n\\r]*";
const SCRIPT_URL = `/^[\\x00-\\x20]*${[..."javascript"].join(URL_GAP)}${URL_GAP}:/i`;
// The sink shapes a property ban cannot see: markup handed over as an object-literal key in any
// letter case (React createElement props or a props object, so the React srcDoc prop too, and
// Object.assign onto an element), the JSX srcDoc prop, an event-handler or srcdoc attribute named
// in a setAttribute or createAttribute method call (or its NS form), a script element named in a
// createElement or createElementNS method call (in the NS form, the local name after any prefix),
// a timer callee neither handler rule reads (the bare name or a window or globalThis member inside
// one wrapper node, or a window or globalThis member named by the head of a template literal),
// whatever its handler and whatever the name is bound to, and a javascript: URL in a string
// literal or the head of a template literal (no-script-url reads only a string literal or an
// expression-free template that starts with javascript:).
const T2_SINK_SELECTORS = [
  "Property[key.name=/^(?:__html|dangerouslySetInnerHTML|innerHTML|outerHTML|srcdoc)$/i]",
  "Property[key.value=/^(?:__html|dangerouslySetInnerHTML|innerHTML|outerHTML|srcdoc)$/i]",
  "JSXAttribute[name.name=/^srcdoc$/i]",
  "CallExpression" +
    memberName("/^(?:set|create)Attribute$/") +
    stringArgument(0, "/^(?:on|srcdoc$)/i"),
  "CallExpression" +
    memberName("/^(?:set|create)AttributeNS$/") +
    stringArgument(1, "/^(?:on|srcdoc$)/i"),
  "CallExpression" + memberName("/^createElement$/") + stringArgument(0, "/^script$/i"),
  "CallExpression" + memberName("/^createElementNS$/") + stringArgument(1, "/(?:^|:)script$/i"),
  `CallExpression[${WRAPPED_CALLEE}.name=${TIMER}]`,
  `CallExpression[${WRAPPED_CALLEE}.object.name=${TIMER_RECEIVER}]` +
    anyOf(propertyName(WRAPPED_CALLEE, TIMER)),
  `CallExpression[callee.object.name=${TIMER_RECEIVER}]` +
    `[callee.property.quasis.0.value.cooked=${TIMER}]`,
  `Literal[value=${SCRIPT_URL}]`,
  `TemplateLiteral[quasis.0.value.cooked=${SCRIPT_URL}]`,
].map((selector) => ({ selector, message: T2_SINK_MESSAGE }));

export default tseslint.config(
  // Not source: build output (Vite + Cargo), the Rust side, deps, the pinned gate tools, and .d.ts
  // shims (src/vite-env.d.ts carries a triple-slash reference the recommended set would flag).
  {
    ignores: [
      "dist/**",
      // `**/target/**` (not root-anchored `target/**`) so a NESTED cargo build dir is excluded too —
      // e.g. `fuzz/target/` from a local `cargo fuzz`/`cargo check` in the standalone fuzz workspace,
      // which emits generated JS (a Tauri `__global-api-script.js`) the ts-gate must not lint (P3.73).
      "**/target/**",
      "src-tauri/**",
      "node_modules/**",
      ".gate-tools/**",
      "**/*.d.ts",
      // The L(-1) gate plane: its JS/TS files are planted-positive fixtures that carry the sinks
      // this config bans on purpose (check-ts-gate lints the T2 one from stdin as src/main.tsx).
      "scripts/**",
    ],
  },
  ...tseslint.configs.recommended,
  {
    // No inline eslint directive takes effect, so none can lift a T2 sink ban; one surfaces as a
    // warning, and check-ts-gate fails the real run on any message.
    linterOptions: { noInlineConfig: true },
    plugins: { react },
    languageOptions: { globals: { ...globals.browser } },
    settings: { react: { version: REACT_VERSION } },
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
        ...T2_SINK_SELECTORS,
      ],
      // G57 leg c: no inline user-facing literal in JSX; UI strings live in src/strings/ui.ts (5.7).
      "react/jsx-no-literals": "error",
      // G57 leg a: ban every i18n-runtime / locale-switch import (English-only, no i18n; section 6.10).
      "no-restricted-imports": ["error", I18N_RESTRICTED],
      // The T2 DOM/eval sink ban (T2_SINK_* above): the sink properties on any receiver and the
      // timers off window and globalThis; the global Function referenced bare, an alias included;
      // eval, indirect eval included; a global timer given a string constant, or a string literal
      // or template alone or in a concatenation; the Function constructor called or applied; a
      // string literal or an expression-free template that starts with javascript:; React
      // dangerouslySetInnerHTML on every component.
      "no-restricted-properties": ["error", ...T2_SINK_PROPERTIES, ...T2_TIMER_PROPERTIES],
      "no-restricted-globals": [
        "error",
        { globals: [{ name: "Function", message: T2_SINK_MESSAGE }], checkGlobalObject: true },
      ],
      "no-eval": ["error", { allowIndirect: false }],
      "no-implied-eval": "error",
      "no-new-func": "error",
      "no-script-url": "error",
      "react/no-danger": ["error", { customComponentNames: ["*"] }],
      "react/no-danger-with-children": "error",
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
  {
    // The type-aware half of the T2 implied-eval ban: over src/ TypeScript a call of the global
    // timer itself, bare or as a dotted or quoted member of window or globalThis, optional
    // chaining included, fails when its handler has a static type that is not a function (a call
    // of a function or method named bind counts as one), a string variable included, unless the
    // scope of the call itself refers to the timer name as a local binding; no-implied-eval reads
    // only a string constant, or a string literal or template alone or in a concatenation. The
    // timer selectors above ban, whatever the handler, a timer callee neither rule reads (inside
    // one wrapper node, or a window or globalThis member named by the head of a template literal).
    // The project service types each file through tsconfig.json.
    files: ["src/**/*.{ts,tsx,mts,cts}"],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      "@typescript-eslint/no-implied-eval": "error",
    },
  },
);
