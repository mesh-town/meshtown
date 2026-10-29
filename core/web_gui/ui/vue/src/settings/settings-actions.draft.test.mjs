/* Tests for the Settings form draft actions: `createSettingsForm`, `editField`, `resetDraft` and `saveDraft`.
 * The output reads as a phrase: `saveDraft > success > draft_empty`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import { createSettingsForm, editField, resetDraft, saveDraft } from "./settings-actions.ts";
import { replyBody, put200, fail, setup, deferred, tick, plain } from "./settings-actions-test-support.mjs";

describe("createSettingsForm / editField / resetDraft", () => {
  it("createSettingsForm — a clean form", () => {
    // The `token` and `timer` fields are not checked.
    const { token, timer, ...planned } = createSettingsForm();
    assert.deepEqual(plain(planned), {
      draft: {},
      errors: {},
      conflict: null,
      message: { text: "", cls: "hint" },
      busy: false,
      restartRequested: false,
      hideFileFieldErrors: false,
      expertOpen: false,
      tokenRevealed: false,
    });
  });

  it("editField writes the raw value to the draft", () => {
    const form = createSettingsForm();
    editField(form, "socks.port", "1081");
    editField(form, "socks.enabled", false);
    assert.deepEqual(plain(form.draft), { "socks.port": "1081", "socks.enabled": false });
  });

  it("resetDraft resets the draft, errors and conflict, but not the expanded state and not the message about something else", () => {
    const form = createSettingsForm();
    editField(form, "socks.port", "1");
    form.errors = { "socks.port": "bad" };
    form.conflict = { rev: 3 };
    form.expertOpen = true;
    resetDraft(form);
    assert.deepEqual(plain(form.draft), {});
    assert.deepEqual(plain(form.errors), {});
    assert.equal(form.conflict, null);
    assert.equal(form.expertOpen, true);
  });
});

describe("saveDraft", () => {
  describe("success", () => {
    it("PUT {rev, config} with the parsed socks.port value 1081", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      const res = await saveDraft(w.form, w.store, w.deps);
      assert.equal(res, "saved");
      assert.equal(w.calls.length, 1);
      assert.equal(w.calls[0].method, "PUT");
      assert.equal(w.calls[0].path, "/api/config");
      assert.equal(w.calls[0].body.rev, 5);
      assert.equal(w.calls[0].body.config.socks.port, 1081);
      assert.equal(w.calls[0].body.config.socks.enabled, true, "the rest of the config went out unchanged");
    });

    it("draft is empty, errors cleared, busy released, file errors hidden", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      w.form.errors = { "socks.port": "old" };
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.draft), {});
      assert.deepEqual(plain(w.form.errors), {});
      assert.equal(w.form.busy, false);
      assert.equal(w.form.hideFileFieldErrors, true);
      assert.equal(w.form.restartRequested, false);
    });

    it("the text \"Saved\", class saved; a field \"after restart\" — the long text", async () => {
      const w = setup({ script: [put200({ restart_fields: ["socks.port"] })] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.message), { text: "Saved. Takes effect after Mesh Town restarts.", cls: "saved" });
    });

    it("a field outside \"after restart\" — just \"Saved\"", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Saved");
    });

    it("one meshtown:changed {what:'config'}, the store updated from the response", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(w.events, [{ what: "config" }]);
      assert.equal(w.store.configRev, 6);
      assert.equal(w.store.config.socks.port, 1081);
    });
  });

  describe("\"Saved\" timer", () => {
    it("after 4000 ms the text is cleared", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.timers.length, 1);
      assert.equal(w.timers[0].ms, 4000);
      w.timers[0].fn();
      assert.equal(w.form.message.text, "");
    });

    it("with a non-empty restart_pending the text stays", async () => {
      const w = setup({ script: [put200({ restart_fields: ["socks.port"], restart_pending: ["socks.port"] })] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      const before = w.form.message.text;
      w.timers[0].fn();
      assert.equal(w.form.message.text, before);
      assert.match(before, /after Mesh Town restarts/);
    });

    it("the text has changed to another one — the timer does not touch it", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      w.form.message = { text: "other", cls: "hint" };
      w.timers[0].fn();
      assert.equal(w.form.message.text, "other");
    });
  });

  describe("skip", () => {
    it("the file is not writable — \"skipped\", 0 requests", async () => {
      const w = setup({ script: [] });
      w.store.configFile = { writable: false, message: "read only" };
      editField(w.form, "socks.port", "1081");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      assert.equal(w.calls.length, 0);
    });

    it("busy — \"skipped\", 0 requests", async () => {
      const w = setup({ script: [] });
      w.form.busy = true;
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      assert.equal(w.calls.length, 0);
    });

    it("a repeated call while the first is in flight — \"skipped\", one request", async () => {
      const gate = deferred();
      const w = setup({ script: [() => gate.promise] });
      editField(w.form, "socks.port", "1081");
      const first = saveDraft(w.form, w.store, w.deps);
      await tick();
      assert.equal(w.form.busy, true);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      gate.resolve(replyBody());
      assert.equal(await first, "saved");
      assert.equal(w.calls.length, 1);
    });
  });

  describe("409 stale_rev", () => {
    it("\"conflict\", conflict.rev from the body, draft intact, no events, store untouched", async () => {
      const w = setup({ script: [fail("stale_rev", 409, { rev: 9, config: {} })] });
      editField(w.form, "socks.port", "1081");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "conflict");
      assert.equal(w.form.conflict.rev, 9);
      assert.deepEqual(plain(w.form.draft), { "socks.port": "1081" });
      assert.deepEqual(w.events, []);
      assert.equal(w.store.configRev, 5);
      assert.equal(w.form.busy, false);
      assert.equal(w.calls.length, 1, "there must be no auto-retry");
    });

    it("retry with form.conflict.rev — the second PUT goes out with this rev, the conflict is cleared", async () => {
      const w = setup({ script: [fail("stale_rev", 409, { rev: 9, config: {} }), put200()] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      const res = await saveDraft(w.form, w.store, w.deps, w.form.conflict.rev);
      assert.equal(res, "saved");
      assert.equal(w.calls[1].body.rev, 9);
      assert.equal(w.form.conflict, null);
      assert.deepEqual(w.events, [{ what: "config" }]);
    });
  });

  describe("422", () => {
    it("checks.provider_dns — errors with the path, a general text, \"For specialists\" expanded", async () => {
      const fields = [{ path: "checks.provider_dns", message: "host:port required" }];
      const w = setup({ script: [fail("invalid", 422, { fields })] });
      editField(w.form, "checks.provider_dns", "junk");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
      assert.deepEqual(plain(w.form.errors), { "checks.provider_dns": "host:port required" });
      assert.deepEqual(plain(w.form.message), { text: "There are errors in the fields — see the notes below.", cls: "field-error-text" });
      assert.equal(w.form.expertOpen, true);
      assert.deepEqual(plain(w.form.draft), { "checks.provider_dns": "junk" }, "draft not lost");
      assert.deepEqual(w.events, []);
    });

    it("an error in a \"Main\" field — \"For specialists\" is not expanded", async () => {
      const fields = [{ path: "socks.port", message: "in use" }];
      const w = setup({ script: [fail("invalid", 422, { fields })] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.expertOpen, false);
    });

    it("not_available — also \"invalid\"", async () => {
      const fields = [{ path: "autostart.enabled", message: "unavailable" }];
      const w = setup({ script: [fail("not_available", 409, { fields })] });
      editField(w.form, "autostart.enabled", true);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
      assert.equal(w.form.errors["autostart.enabled"], "unavailable");
    });

    it("not_available — the next successful save clears the error", async () => {
      const fields = [{ path: "autostart.enabled", message: "unavailable" }];
      const w = setup({ script: [fail("not_available", 409, { fields }), put200()] });
      editField(w.form, "autostart.enabled", true);
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.errors["autostart.enabled"], "unavailable");
      editField(w.form, "autostart.enabled", false);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(plain(w.form.errors), {});
    });

    it("a new attempt replaces the previous errors", async () => {
      const w = setup({ script: [fail("invalid", 422, { fields: [{ path: "socks.port", message: "in use" }] }), put200()] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.errors), {});
    });
  });

  describe("other failures", () => {
    it("config_write_failed — text from the body or a fallback", async () => {
      const w = setup({ script: [fail("config_write_failed", 500, { message: "disk full" }), fail("config_write_failed", 500, null)] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.deepEqual(plain(w.form.message), { text: "Couldn't save: disk full", cls: "field-error-text" });
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Couldn't write the settings file");
    });

    it("500 — the body text; busy released; draft intact; no events", async () => {
      const w = setup({ script: [fail("boom", 500, { message: "node failure" })] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.deepEqual(plain(w.form.message), { text: "Couldn't save: node failure", cls: "field-error-text" });
      assert.equal(w.form.busy, false);
      assert.deepEqual(plain(w.form.draft), { "socks.port": "1" });
      assert.deepEqual(w.events, []);
    });

    it("network — the core error text", async () => {
      const w = setup({ script: [() => { throw new ApiError(0, "network", API_ERROR_TEXT.network); }] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.match(w.form.message.text, /^Couldn't save: /);
      assert.equal(w.form.busy, false);
    });

    it("an error without text — \"Couldn't save — try again\"", async () => {
      const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x", status: 500, body: null }); }] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Couldn't save — try again");
    });
  });
});
