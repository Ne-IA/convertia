import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";

// The section 6.4.6a / G33a ATTRIBUTABLE a11y leg (P1.56): the same jsdom + SWC-react setup as the
// general suite (vitest.config.ts), narrowed to the a11y test files (*.a11y.test.{ts,tsx}) and run by a
// dedicated `pnpm test:a11y` CI step, so an ARIA / role / focus regression is its OWN G33a red instead of
// being buried in the general unit run. Colour contrast is NOT here -- jsdom cannot compute it; that is
// the Lane-B headed @axe-core/webdriverio leg (section 6.4.6a). [Build-Session-Entscheidung: P1.56]
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.a11y.test.{ts,tsx}"],
    // parity with vitest.config.ts: spread the vitest defaults so node_modules/dist/etc. stay excluded
    // (the positive include already scopes the run, but this future-proofs against new default excludes).
    exclude: [...configDefaults.exclude],
    css: false,
  },
});
