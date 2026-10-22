/* `saveDraft`: what saving does to the draft if input happened while the PUT was in flight.
 * The output reads as a phrase: `saveDraft > input_during_PUT > is_not_erased`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createSettingsForm, editField, saveDraft } from "./settings-actions.ts";

// `sent` — the key that went into the PUT; `later` — the key typed after sending.
const SENT = "checks.enabled";
const LATER = "socks.port";

describe("saveDraft", () => {
  describe("input during PUT", () => {
    it("is not erased: both the edit of the sent field and the new field remain", async () => {
      // given
      const w = setup();
      editField(w.form, SENT, false);
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, SENT, true);
      editField(w.form, LATER, "1090");
      w.reply.resolve();
      assert.equal(await saving, "saved");
      // then
      assert.deepEqual(plain(w.form.draft), { [SENT]: true, [LATER]: "1090" });
    });

    it("the \"Saved\" text depends on the sent keys, not on those typed later", async () => {
      // given: `socks.port` requires a restart, `checks.enabled` does not
      const w = setup();
      editField(w.form, SENT, false);
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, LATER, "1090");
      w.reply.resolve();
      await saving;
      // then
      assert.equal(w.form.message.text, "Saved");
    });

    it("PUT failed: the draft is intact together with the input made during the PUT", async () => {
      const w = setup();
      editField(w.form, SENT, false);
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, LATER, "1090");
      w.reply.reject(new ApiError(0, "network"));
      assert.notEqual(await saving, "saved");
      assert.deepEqual(plain(w.form.draft), { [SENT]: false, [LATER]: "1090" });
    });
  });

  describe("no input", () => {
    it("draft is empty", async () => {
      // given
      const w = setup();
      editField(w.form, SENT, false);
      editField(w.form, LATER, "1090");
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      w.reply.resolve();
      await saving;
      // then
      assert.deepEqual(plain(w.form.draft), {});
    });
  });
});

/** The PUT hangs until `reply.resolve()` is called; `socks.port` — "after restart". */
function setup() {
  const store = createStore();
  store.config = {
    socks: { enabled: true, port: 1080 },
    checks: { enabled: true, provider_dns: null },
    autostart: { available: true, reason: "", enabled: false },
  };
  store.configRev = 5;
  store.configMeta = { read_only: [], restart_fields: [LATER], restart_pending: [], cli_overrides: [] };
  store.configFile = null;
  const reply = deferred();
  const api = async (method, path, body) => {
    await reply.promise;
    return { config: structuredClone(body.config), rev: body.rev + 1, restart_fields: [LATER] };
  };
  const deps = { api, busTarget: new EventTarget(), setTimer: () => 1, clearTimer: () => {} };
  return { store, form: createSettingsForm(), deps, reply };
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => ((resolve = res), (reject = rej)));
  return { promise, resolve, reject };
}

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
