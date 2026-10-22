/* The `useDiagnostics` watcher: the network key is reset after a change of `network.id_hex` and when the
 * network disappears. A fake store and `api`, `withSetup` instead of a component. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { useDiagnostics } from "./use-diagnostics.ts";

describe("useDiagnostics > network key", () => {
  it("a shown key stays visible while the network is the same", async () => {
    const w = await setupShown();
    await nextTick();
    assert.equal(w.d.networkKeyState.value.shown, true);
    assert.equal(w.d.networkKeyState.value.full, "mesh1-FULLKEY");
    w.unmount();
  });

  it("network.id_hex changes after showing — key.shown false, full null", async () => {
    const w = await setupShown();
    w.store.state = withNetwork("id-bbbb");
    await nextTick();
    assert.equal(w.d.networkKeyState.value.shown, false, "the key watch did not react to the network change");
    assert.equal(w.d.networkKeyState.value.full, null);
    assert.equal(w.d.networkKeyState.value.netId, null);
    w.unmount();
  });

  it("the network disappeared — the key is reset", async () => {
    const w = await setupShown();
    w.store.state = { ...withNetwork("id-aaaa"), network: null };
    await nextTick();
    assert.equal(w.d.networkKeyState.value.shown, false);
    assert.equal(w.d.networkKeyState.value.full, null);
    w.unmount();
  });

  it("a poll with the same id_hex does not touch the key", async () => {
    const w = await setupShown();
    w.store.state = withNetwork("id-aaaa");
    await nextTick();
    assert.equal(w.d.networkKeyState.value.shown, true);
    w.unmount();
  });
});

describe("useDiagnostics > views", () => {
  it("checks and events follow the snapshot", async () => {
    const w = setup();
    assert.ok(w.d.checks.value.length >= 3);
    assert.equal(w.d.events.value.length, 1);
    w.store.state = { ...withNetwork("id-aaaa"), log: { tail: [], last_seq: 0 } };
    await nextTick();
    assert.deepEqual(w.d.events.value, []);
    w.unmount();
  });
});

function withNetwork(idHex) {
  return {
    node: { status: "online", uptime_sec: 60, version: "1.2.3" },
    network: { label: "home", masked: "mesh1-aa", id_hex: idHex },
    log: { tail: [{ seq: 1, t: 1, tag: "x", text: "y" }], last_seq: 1 },
  };
}

async function setupShown() {
  const w = setup();
  assert.equal(await w.d.toggleKey(), "shown");
  return w;
}

function setup() {
  const store = createStore();
  store.state = withNetwork("id-aaaa");
  const deps = {
    api: async () => ({ key: "mesh1-FULLKEY" }),
    clipboard: null,
    setTimer: () => 1,
    clearTimer: () => {},
  };
  const { result: d, unmount } = withSetup(() => useDiagnostics(store, deps));
  return { d, store, unmount };
}

describe("useDiagnostics > keyText", () => {
  it("the mask, after \"Show\" the full key, after \"Hide\" the mask, a network change — the mask of the new network", async () => {
    const w = setup();
    assert.equal(w.d.keyText.value, "mesh1-aa");
    assert.equal(await w.d.toggleKey(), "shown");
    assert.equal(w.d.keyText.value, "mesh1-FULLKEY");
    assert.equal(await w.d.toggleKey(), "hidden");
    assert.equal(w.d.keyText.value, "mesh1-aa");
    await w.d.toggleKey();
    w.store.state = { ...withNetwork("id-bbbb"), network: { label: "w", masked: "mesh1-bb", id_hex: "id-bbbb" } };
    await nextTick();
    assert.equal(w.d.keyText.value, "mesh1-bb");
    w.unmount();
  });
});
