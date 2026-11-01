/* Tests for a refused save: `not_available` takes the refused paths out of the draft, `invalid` keeps it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { editField, saveDraft } from "./settings-actions.ts";
import { fail, setup, plain } from "./settings-actions-test-support.mjs";

describe("saveDraft > refused paths", () => {
  it("not_available_on_autostart_drops_the_draft_and_keeps_the_error", async () => {
    const fields = [{ path: "autostart.enabled", message: "could not turn on start at login: denied" }];
    const w = setup({ script: [fail("not_available", 422, { fields })] });
    editField(w.form, "autostart.enabled", true);
    assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
    assert.equal("autostart.enabled" in w.form.draft, false, "the switch goes back to the config value");
    assert.equal(w.form.errors["autostart.enabled"], "could not turn on start at login: denied");
  });

  it("invalid_keeps_the_draft", async () => {
    const fields = [{ path: "socks.port", message: "in use" }];
    const w = setup({ script: [fail("invalid", 422, { fields })] });
    editField(w.form, "socks.port", "1");
    assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
    assert.deepEqual(plain(w.form.draft), { "socks.port": "1" });
    assert.equal(w.form.errors["socks.port"], "in use");
  });

  it("not_available_drops_only_the_listed_paths", async () => {
    const fields = [{ path: "autostart.enabled", message: "denied" }];
    const w = setup({ script: [fail("not_available", 422, { fields })] });
    editField(w.form, "autostart.enabled", true);
    editField(w.form, "node.name", "alpha");
    await saveDraft(w.form, w.store, w.deps);
    assert.deepEqual(plain(w.form.draft), { "node.name": "alpha" });
  });
});
