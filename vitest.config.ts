import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";

// The section 6.4.6a Vitest jsdom test leg (G33a): renders the React tree under jsdom and runs
// vitest-axe over it (ARIA / role / focus -- NOT colour contrast, which jsdom cannot compute; that
// is the Lane-B headed @axe-core/webdriverio leg). Kept separate from vite.config.ts so the build
// config stays pure; the SWC react plugin is shared so test JSX/TSX transforms the same way the
// build does. [Build-Session-Entscheidung: P1.35]
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    css: false,
  },
});
