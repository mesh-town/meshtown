/* Copying the PAC address and the form of the "Connect the browser" card (`home-actions.ts`):
 * `copyValue(form, "pac", …)` — "Copied" and a clipboard refusal with its own note, `noteKind` —
 * whose note is in the form now (the card hides it for a foreign kind); the initial form with `os` and `noteKind`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { PAC_COPY_FALLBACK } from "../common/pac-texts.ts";
import { copyValue, createHomeForm } from "./home-actions.ts";

const PAC_URL = "http://127.0.0.1:1080/proxy.pac";

describe("createHomeForm, the browser card", () => {
  it("no system chosen (os: null), no foreign note (noteKind: \"\"), the block is collapsed", () => {
    assert.deepEqual(createHomeForm().browser, {
      choice: null, os: null, open: false, copied: "", note: "", noteKind: "",
    });
  });
});

describe("copyValue, the PAC address", () => {
  describe("the clipboard accepted", () => {
    it("\"copied\", the address is in the clipboard, \"Copied\" at pac and goes out after 2000 ms", async () => {
      const w = setup({});
      assert.equal(await copyValue(w.form, "pac", PAC_URL, w.deps), "copied");
      assert.deepEqual(w.written, [PAC_URL]);
      assert.equal(w.form.browser.copied, "pac");
      assert.equal(w.timers[0].ms, 2000);
      w.timers[0].fn();
      assert.equal(w.form.browser.copied, "");
    });

    it("the note is empty, but noteKind = \"pac\"", async () => {
      const w = setup({});
      await copyValue(w.form, "pac", PAC_URL, w.deps);
      assert.equal(w.form.browser.note, "");
      assert.equal(w.form.browser.noteKind, "pac");
    });

    it("the previous refusal note is erased", async () => {
      const w = setup({ clipboard: null });
      await copyValue(w.form, "pac", PAC_URL, w.deps);
      w.deps.clipboard = w.working;
      await copyValue(w.form, "pac", PAC_URL, w.deps);
      assert.equal(w.form.browser.note, "");
    });
  });

  describe("no clipboard", () => {
    it("\"fallback\", \"Copied\" is not shown, the note is PAC_COPY_FALLBACK, noteKind \"pac\"", async () => {
      const w = setup({ clipboard: null });
      assert.equal(await copyValue(w.form, "pac", PAC_URL, w.deps), "fallback");
      assert.equal(w.form.browser.copied, "");
      assert.equal(w.form.browser.note, PAC_COPY_FALLBACK);
      assert.equal(w.form.browser.noteKind, "pac");
    });

    it("there are no words \"in the step\" in the note — the PAC address is not in a step", async () => {
      const w = setup({ clipboard: null });
      await copyValue(w.form, "pac", PAC_URL, w.deps);
      assert.ok(!w.form.browser.note.includes("in the step"), w.form.browser.note);
    });
  });

  describe("the clipboard refused (writeText threw)", () => {
    it("\"fallback\" with the same PAC_COPY_FALLBACK note", async () => {
      const w = setup({ clipboard: { writeText: async () => { throw new Error("denied"); } } });
      assert.equal(await copyValue(w.form, "pac", PAC_URL, w.deps), "fallback");
      assert.equal(w.form.browser.copied, "");
      assert.equal(w.form.browser.note, PAC_COPY_FALLBACK);
      assert.equal(w.form.browser.noteKind, "pac");
    });
  });
});

describe("copyValue, other kinds on a clipboard refusal — the text is as before", () => {
  const cases = [
    ["host", "127.0.0.1", "Clipboard unavailable — select the address in the step and copy it manually."],
    ["port", "1080", "Clipboard unavailable — select the port in the step and copy it manually."],
    ["lan", "192.168.8.1:1080", "Clipboard unavailable — select the address for other devices in the step and copy it manually."],
  ];
  for (const [kind, value, note] of cases) {
    it(`${kind}: note "${note}", noteKind "${kind}"`, async () => {
      const w = setup({ clipboard: null });
      assert.equal(await copyValue(w.form, kind, value, w.deps), "fallback");
      assert.equal(w.form.browser.note, note);
      assert.equal(w.form.browser.noteKind, kind);
    });
  }
});

describe("copyValue, noteKind", () => {
  it("success of another kind — noteKind moves to it, the note is empty", async () => {
    const w = setup({});
    await copyValue(w.form, "pac", PAC_URL, w.deps);
    await copyValue(w.form, "port", "1080", w.deps);
    assert.equal(w.form.browser.noteKind, "port");
    assert.equal(w.form.browser.note, "");
  });

  it("pac refusal, then host refusal — the note and noteKind are from the last one", async () => {
    const w = setup({ clipboard: null });
    await copyValue(w.form, "pac", PAC_URL, w.deps);
    await copyValue(w.form, "host", "127.0.0.1", w.deps);
    assert.equal(w.form.browser.noteKind, "host");
    assert.ok(w.form.browser.note.includes("in the step"), "the host kind's note is the previous one, about the step");
  });
});

// Preconditions: the clipboard (by default writing into `written`; null — no clipboard; its own object — as is).
function setup({ clipboard }) {
  const written = [];
  const timers = [];
  const working = { writeText: async (t) => { written.push(t); } };
  const deps = {
    clipboard: clipboard === undefined ? working : clipboard,
    setTimer: (fn, ms) => { timers.push({ fn, ms }); return timers.length; },
    clearTimer: () => {},
  };
  return { form: createHomeForm(), deps, written, timers, working };
}
