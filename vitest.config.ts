import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";

// The section 6.4.6a Vitest jsdom test leg: renders the React tree under jsdom. Kept separate from
// vite.config.ts so the build config stays pure; the SWC react plugin is shared so test JSX/TSX transforms
// the same way the build does. [Build-Session-Entscheidung: P1.35]
//
// The a11y tests (*.a11y.test.{ts,tsx}) are EXCLUDED here and run in the dedicated, attributable G33a leg
// (vitest.a11y.config.ts / `pnpm test:a11y`), so an ARIA / role / focus regression is its own red rather
// than buried in this general unit run. configDefaults.exclude is spread so node_modules/dist/etc. stay
// excluded. (Colour contrast is neither here nor in the a11y leg -- jsdom cannot compute it; that is the
// Lane-B headed @axe-core/webdriverio leg.) [Build-Session-Entscheidung: P1.56]
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: [...configDefaults.exclude, "**/*.a11y.test.{ts,tsx}"],
    css: false,
  },
});
