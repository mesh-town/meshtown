/* Clipboard helper: success, no clipboard, clipboard refusal. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { writeClipboard } from "./clipboard.ts";

describe("writeClipboard", () => {
  it("text went to the clipboard — true", async () => {
    const written = [];
    const deps = { clipboard: { writeText: async (t) => { written.push(t); } } };
    assert.equal(await writeClipboard(deps, "abc"), true);
    assert.deepEqual(written, ["abc"]);
  });

  it("no clipboard — false", async () => {
    assert.equal(await writeClipboard({ clipboard: null }, "abc"), false);
  });

  it("clipboard refused — false, no exception", async () => {
    const deps = { clipboard: { writeText: async () => { throw new Error("denied"); } } };
    assert.equal(await writeClipboard(deps, "abc"), false);
  });
});
