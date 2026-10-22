/* One runner for the page, two projects. `node`: the logic of `*.test.mjs` in the node environment; it does NOT extend
 * `vite.config.js`, so the vue plugin is not there and importing a `.vue` file fails — this is the "logic without Vue and DOM" boundary.
 * `dom`: the `*.vtest.mjs` components in happy-dom on top of the build config (`vite.config.js`, the vue plugin).
 * `pool`, `isolate`, `globals` are not set: the defaults are needed (one process per file). */

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    projects: [
      {
        test: {
          name: "node",
          environment: "node",
          include: ["src/**/*.test.mjs"],
          restoreMocks: true,
          unstubGlobals: true,
        },
      },
      {
        extends: "./vite.config.js",
        test: {
          name: "dom",
          environment: "happy-dom",
          include: ["src/**/*.vtest.mjs"],
          restoreMocks: true,
          unstubGlobals: true,
        },
      },
    ],
  },
});
