/* Guard test for the word "node": string literals of the page where the program is the subject of the phrase
 * say `NET_NAME` (`words.ts`), not "Node ...".
 * A file-based check: the `.ts|.vue` sources under `src/` without `*.test.*` and `*.vtest.*`.
 * Descriptive phrases ("Node log", "node page") are allowed: only a literal that STARTS with "Node" as a subject
 * and a literal with "node erased" are caught. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SRC_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SOURCE_FILE = /\.(ts|vue)$/;
const TEST_FILE = /\.(test|vtest)\./;
// A literal opens with a quote, and right after it comes "Node" followed by a verb (the subject role).
// "The node ..." / "Node ..." / "node ..." followed by a word is a subject, except the descriptive nouns listed in the lookahead.
const LITERAL_STARTS_WITH_NODE = /["'`](?:The )?[Nn]ode (?!log\b|page\b|web\b|identity\b|token\b|settings\b)[a-z']+/;
// A quote, then no other quotes, then "node erased" (the log gap text: `LOG_GAP_TEXT`).
const LITERAL_HAS_NODE_ERASED = /["'`][^"'`]*\bnode erased\b/;

describe('the word "node" in page literals', () => {
  it("the walk sees the page sources (otherwise the check is empty)", () => {
    const files = sourceFiles().map((f) => f.rel);
    assert.ok(files.includes("shell/status-line.ts"), "no shell/status-line.ts among " + files.length + " files");
    assert.ok(files.includes("core/api.ts"), "no core/api.ts among " + files.length + " files");
  });

  it("the patterns catch a subject and skip a descriptive phrase", () => {
    assert.ok(LITERAL_STARTS_WITH_NODE.test('"Node is not responding"'));
    assert.ok(LITERAL_STARTS_WITH_NODE.test('"The node is not responding"'));
    assert.ok(LITERAL_STARTS_WITH_NODE.test('"node restarts"'));
    assert.ok(!LITERAL_STARTS_WITH_NODE.test('"Node log"'));
    assert.ok(!LITERAL_STARTS_WITH_NODE.test('"The node page is protected"'));
    assert.ok(LITERAL_HAS_NODE_ERASED.test('"lines skipped: node erased them"'));
  });

  it("no literal starts with \"Node\" as a subject", () => {
    const found = findLines((_rel, line) => LITERAL_STARTS_WITH_NODE.test(line));
    assert.deepEqual(found, [], "instead of \"Node ...\" use NET_NAME + \" ...\": " + found.join("; "));
  });

  it("no literal with \"node erased\"", () => {
    const found = findLines((_rel, line) => LITERAL_HAS_NODE_ERASED.test(line));
    assert.deepEqual(found, [], "instead of \"node erased\" use NET_NAME + \" erased ...\": " + found.join("; "));
  });
});

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const full = path.join(dir, e.name);
    return e.isDirectory() ? walk(full) : [full];
  });
}

// Page sources without test files: `{ full, rel }`, `rel` is relative to `src/` with slashes.
function sourceFiles() {
  return walk(SRC_DIR)
    .filter((f) => SOURCE_FILE.test(f) && !TEST_FILE.test(path.basename(f)))
    .map((full) => ({ full, rel: path.relative(SRC_DIR, full).split(path.sep).join("/") }));
}

// Source lines where `bad(rel, line)` is true, as "file:line: text".
function findLines(bad) {
  return sourceFiles().flatMap(({ full, rel }) =>
    fs.readFileSync(full, "utf8").split("\n")
      .map((line, i) => ({ line, where: `${rel}:${i + 1}` }))
      .filter(({ line }) => bad(rel, line))
      .map(({ line, where }) => `${where}: ${line.trim()}`),
  );
}
