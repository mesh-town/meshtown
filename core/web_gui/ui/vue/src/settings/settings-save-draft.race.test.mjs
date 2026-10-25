/* `saveDraft` right after a node-name save: the second write waits for the first and is not seen as a conflict. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createHomeForm, saveName } from "../home/home-actions.ts";
import { createSettingsForm, editField, saveDraft } from "./settings-actions.ts";

function setup() {
  const store = createStore();
  store.config = { node: { name: "old" }, checks: { enabled: true } };
  store.configRev = 5;
  store.configMeta = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
  store.configFile = null;
  const server = { config: { node: { name: "old" }, checks: { enabled: true } }, rev: 5 };
  const calls = [];
  const api = async (method, path, body) => {
    calls.push({ method, path, rev: body && body.rev });
    if (body.rev !== server.rev) throw new ApiError(409, "stale_rev", "stale", { rev: server.rev });
    server.config = structuredClone(body.config);
    server.rev += 1;
    const reply = { config: structuredClone(server.config), rev: server.rev };
    await new Promise((r) => setTimeout(r, 10));
    return reply;
  };
  const deps = { api, busTarget: new EventTarget(), setTimer: () => 1, clearTimer: () => {} };
  return { store, deps, calls, server, form: createSettingsForm(), home: createHomeForm() };
}

async function nameThenSave(w, edit) {
  w.home.name.draft = "new";
  w.home.name.original = "old";
  const naming = saveName(w.home, w.store, w.deps);
  if (edit) editField(w.form, "checks.enabled", false);
  const saving = saveDraft(w.form, w.store, w.deps);
  return { naming: await naming, outcome: await saving };
}

describe("saveDraft after saveName", () => {
  it("two PUTs, the second with the revision from the first reply, no conflict", async () => {
    const w = setup();
    const { naming, outcome } = await nameThenSave(w, true);
    assert.equal(naming, "saved");
    assert.equal(outcome, "saved");
    assert.equal(w.form.conflict, null);
    assert.deepEqual(w.calls.map((c) => c.rev), [5, 6]);
    assert.equal(w.server.config.node.name, "new");
    assert.equal(w.server.config.checks.enabled, false);
  });

  it("an empty draft: still saved, no conflict", async () => {
    const w = setup();
    const { outcome } = await nameThenSave(w, false);
    assert.equal(outcome, "saved");
    assert.equal(w.form.conflict, null);
    assert.deepEqual(w.calls.map((c) => c.rev), [5, 6]);
  });
});
