/* Size limits and bans in the page sources.
 * A file check: no target, browser or DOM needed. The sources are `.js|.ts|.vue` under `src/`, except `*.test.*` and
 * `*.vtest.*` (the check survives moving `src` to TS). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SRC_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_DIR = path.resolve(SRC_DIR, "..", "..");
// The size ceiling is a warning, not a failure.
const SIZE_LIMIT_BYTES = 300_000;
// The Vue template compiler in the build is `Function(...)`, i.e. `unsafe-eval`, which CSP forbids.
// A bare `Function(` does not catch `isFunction(` (there are dozens in runtime-core).
const COMPILER_MARK = /(?<![\w$.])Function\(/;
const SOURCE_FILE = /\.(js|ts|vue)$/;
const TEST_FILE = /\.(test|vtest)\./;

const walk = (dir) =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const full = path.join(dir, e.name);
    return e.isDirectory() ? walk(full) : [full];
  });

/** Lines of the `web/vue/src/**` sources (without test files) labeled "file:line". */
function vueSourceLines() {
  return walk(SRC_DIR)
    .filter((f) => SOURCE_FILE.test(f) && !TEST_FILE.test(path.basename(f)))
    .flatMap((f) =>
      fs.readFileSync(f, "utf8").split("\n").map((line, i) => ({ line, where: `${path.relative(WEB_DIR, f)}:${i + 1}` })),
    );
}

const filesWith = (dir, ext) =>
  fs.existsSync(path.join(WEB_DIR, dir))
    ? fs.readdirSync(path.join(WEB_DIR, dir)).filter((n) => n.endsWith(ext)).sort().map((n) => `${dir}/${n}`)
    : [];

const strayLines = (test) =>
  vueSourceLines().filter(({ line }) => test(line)).map(({ line, where }) => `${where}: ${line.trim()}`);

describe("14-static-limits: size of embedded files, innerHTML and window", () => {
  it("14-static-limits: size of index.html + css + bundle within the ceiling (above it — a warning)", () => {
    const names = ["index.html", ...filesWith("css", ".css"), ...filesWith("bundle", ".js")];
    const sizes = names.map((n) => fs.statSync(path.join(WEB_DIR, n)).size);
    const total = sizes.reduce((a, b) => a + b, 0);
    if (total <= SIZE_LIMIT_BYTES) return;
    const message = `total ${total} of ${SIZE_LIMIT_BYTES}: ${names.map((n, i) => `${n} ${sizes[i]}`).join(", ")}`;
    console.warn("[14-static-limits] warning, size above the ceiling — " + message);
  });

  it("14-static-limits: no template compiler (Function( ) in bundle/app.js", () => {
    const bundle = path.join(WEB_DIR, "bundle", "app.js");
    assert.ok(fs.existsSync(bundle), "no built bundle/app.js — make web-build");
    const text = fs.readFileSync(bundle, "utf8");
    const at = text.search(COMPILER_MARK);
    assert.equal(at, -1, `the build has Function( — the template compiler (unsafe-eval): ...${text.slice(at, at + 60)}...`);
  });

  it("14-static-limits: no innerHTML in the vue/src sources", () => {
    assert.deepEqual(strayLines((line) => line.includes("innerHTML")), [], "innerHTML in vue/src");
  });

  it("14-static-limits: no window/globalThis in the vue/src sources without exceptions", () => {
    const stray = strayLines((line) => line.includes("window.") || line.includes("globalThis."));
    assert.deepEqual(stray, [], "accesses to window/globalThis in vue/src");
  });
});
