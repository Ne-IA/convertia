import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";

// The §6.7.1 step-3 COVERAGE run (G27 per-domain line/branch floors + G28 diff gate, P1.54). Distinct from
// vitest.config.ts (the fast G5/G6/G13 general unit run, a11y excluded) and vitest.a11y.config.ts (the
// attributable G33a a11y run): coverage must reflect the WHOLE suite, so `include` here carries NO a11y
// exclusion — `src/**/*.test.{ts,tsx}` matches both the general tests AND `*.a11y.test.tsx` (so e.g.
// App.tsx earns the render coverage its a11y test exercises). Same jsdom + SWC-react setup as the build.
// [Build-Session-Entscheidung: P1.54]
//
// Emits exactly the two machine-readable reports check-coverage (G27/G28) reads on the Linux Lane-A leg
// (NOT the human `lcov` HTML report — `lcovonly`, so coverage/ holds no .js/.html to lint/format):
//   * json-summary -> coverage/coverage-summary.json  — per-FILE summary, the TS per-domain (G27) source
//   * lcovonly     -> coverage/lcov.info              — per-LINE DA records, the TS diff (G28) source
// `all: true` counts every src product file (even untested ones) so the package floor is honest, not a
// vacuous "only the touched files" number. Generated (bindings.ts), the bootstrap entry (main.tsx) and
// type-declaration files are not product-to-cover and are excluded. coverage/ is gitignored + prettier-
// ignored (a generated artifact).
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: [...configDefaults.exclude],
    css: false,
    coverage: {
      provider: "v8",
      reporter: ["json-summary", "lcovonly"],
      reportsDirectory: "coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/main.tsx",
        "src/lib/ipc/bindings.ts",
        "src/vite-env.d.ts",
        "**/*.d.ts",
      ],
      all: true,
    },
  },
});
