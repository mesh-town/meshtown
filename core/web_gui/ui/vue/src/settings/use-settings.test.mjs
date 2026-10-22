/* `useSettings` watchers: `store.settingsDirty` following the draft, and clearing the restart request after
 * `restart_pending` empties. A mock store, `withSetup` instead of a component. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { RESTART_STARTED } from "./settings-view.ts";
import { useSettings } from "./use-settings.ts";

describe("useSettings > \"unsaved\" label", () => {
  it("an edit sets store.settingsDirty, cancel clears it", async () => {
    const w = setup();
    assert.equal(w.store.settingsDirty, false);
    w.s.edit("socks.port", "1081");
    await nextTick();
    assert.equal(w.store.settingsDirty, true);
    w.s.reset();
    await nextTick();
    assert.equal(w.store.settingsDirty, false);
    w.unmount();
  });
});

describe("useSettings > restart request", () => {
  it("restart_pending became empty — restartRequested is cleared, the message removed", async () => {
    const w = setup({ pending: ["socks.port"] });
    w.s.form.restartRequested = true;
    w.s.form.message = { text: RESTART_STARTED, cls: "saved" };
    await nextTick();
    assert.equal(w.s.form.restartRequested, true);
    w.store.configMeta = meta([]);
    await nextTick();
    assert.equal(w.s.form.restartRequested, false);
    assert.equal(w.s.form.message.text, "");
    w.unmount();
  });

  it("the POST response arrived when restart_pending was already empty — the request is cleared anyway", async () => {
    const w = setup({ pending: [] });
    await nextTick();
    await w.s.restart();
    await nextTick();
    assert.equal(w.s.form.restartRequested, false);
    assert.equal(w.s.form.message.text, "");
    w.unmount();
  });

  it("while restart_pending is non-empty, the request holds", async () => {
    const w = setup({ pending: ["socks.port"] });
    await w.s.restart();
    await nextTick();
    assert.equal(w.s.form.restartRequested, true);
    assert.equal(w.s.form.message.text, RESTART_STARTED);
    w.unmount();
  });
});

function meta(pending) {
  return { read_only: [], restart_fields: [], restart_pending: pending, cli_overrides: [] };
}

function setup({ pending = [] } = {}) {
  const store = createStore();
  store.config = { socks: { port: 1080 } };
  store.configRev = 1;
  store.configMeta = meta(pending);
  const deps = { api: async () => ({ restart: true }), setTimer: () => 1, clearTimer: () => {} };
  const { result: s, unmount } = withSetup(() => useSettings(store, deps));
  return { s, store, unmount };
}
