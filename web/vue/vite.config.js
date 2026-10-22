/* Page build: src/main.ts → ../bundle/app.js, a single file. The output directory is outside the Vite
 * root; `make` sets it with the flags `--outDir … --emptyOutDir` (see `web-build`, `web-bundle-check`),
 * it is not read from the environment. */

import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [
    vue({
      features: { optionsAPI: false, prodDevtools: false, prodHydrationMismatchDetails: false },
    }),
  ],
  publicDir: false,
  build: {
    sourcemap: false,
    modulePreload: false,
    manifest: false,
    rolldownOptions: {
      input: fileURLToPath(new URL("./src/main.ts", import.meta.url)),
      output: { format: "es", entryFileNames: "app.js", codeSplitting: false },
    },
  },
});
