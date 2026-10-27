/* `useSettings`: the restart request after a failed start is cleared once the node reports the outcome of the new attempt. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { useSettings } from "./use-settings.ts";

const ERR = "port 1091 for the browser proxy is in use";
const state = (bootId, startError) => ({ boot_id: bootId, node: { start_error: startError } });

describe("useSettings > restart after a failed start", () => {
  it("start_error cleared and set again — the button is offered, the request is cleared", async () => {
    const w = setup();
    await w.s.restart();
    await nextTick();
    assert.equal(w.s.form.restartRequested, true);
    assert.equal(w.s.bar.value.restartOffered, false);
    w.store.state = state("b1", null);
    await nextTick();
    w.store.state = state("b2", ERR);
    await nextTick();
    assert.equal(w.s.form.restartRequested, false);
    assert.equal(w.s.bar.value.restartOffered, true);
    w.unmount();
  });

  it("a new attempt failed between two polls (new boot_id, start_error never seen empty) — the request is cleared", async () => {
    const w = setup();
    await w.s.restart();
    w.store.state = state("b2", ERR);
    await nextTick();
    assert.equal(w.s.form.restartRequested, false);
    assert.equal(w.s.form.message.text, "");
    assert.equal(w.s.bar.value.restartOffered, true);
    w.unmount();
  });

  it("the stale start_error of the previous attempt (same boot_id) does not clear the request", async () => {
    const w = setup();
    await w.s.restart();
    w.store.state = state("b1", ERR);
    await nextTick();
    assert.equal(w.s.form.restartRequested, true);
    assert.equal(w.s.bar.value.restartOffered, false);
    w.unmount();
  });
});

function setup() {
  const store = createStore();
  store.config = { socks: { port: 1080 } };
  store.configRev = 1;
  store.state = state("b1", ERR);
  const deps = { api: async () => ({ restart: true }), setTimer: () => 1, clearTimer: () => {} };
  const { result: s, unmount } = withSetup(() => useSettings(store, deps));
  return { s, store, unmount };
}
