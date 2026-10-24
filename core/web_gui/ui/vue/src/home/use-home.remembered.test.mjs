/* `useHome` on one network: which remembered exit ids are shown as candidates. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { REMEMBERED_KEY } from "../common/exits.ts";
import { useHome } from "./use-home.ts";

const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/126.0.0.0 Safari/537.36";
const STRANGER = "x".repeat(64);

describe("useHome > remembered exits on one network", () => {
  it("a legacy bare id of a stranger is not a candidate", () => {
    const w = setup({ peers: [] }, { [REMEMBERED_KEY]: JSON.stringify([STRANGER]) });
    assert.equal(w.h.candidates.value.some((c) => c.idFull === STRANGER), false);
    w.unmount();
  });

  it("an entry written by this page stays a candidate", () => {
    const w = setup({ peers: [] }, { [REMEMBERED_KEY]: JSON.stringify([{ id: STRANGER, slot: "main" }]) });
    assert.equal(w.h.candidates.value.some((c) => c.idFull === STRANGER), true);
    w.unmount();
  });

  it("a legacy bare id of a peer that offers an exit stays", () => {
    const peer = { id: "b".repeat(10), id_full: "b".repeat(64), name: "b", online: true, exit: true, rtt_ms: 10 };
    const w = setup({ peers: [peer] }, { [REMEMBERED_KEY]: JSON.stringify(["b".repeat(64)]) });
    assert.equal(w.h.candidates.value.some((c) => c.idFull === "b".repeat(64)), true);
    w.unmount();
  });
});

function setup(state, data) {
  const store = createStore();
  store.state = { node: { status: "online", id: "aaaa1111" }, network: { label: "home", id_hex: "n0" }, ...state };
  store.config = { node: { name: "n" }, route: { exits: [] }, exit: { enabled: false } };
  store.configRev = 1;
  const api = async (method, path, body) => ({ rev: 2, config: body?.config ?? store.config });
  const storage = { getItem: (k) => data[k] ?? null, setItem: (k, v) => { data[k] = v; } };
  const deps = { api, clipboard: null, setTimer: () => 1, clearTimer: () => {}, storage, userAgent: DESKTOP_UA, focus: () => {} };
  const { result: h, unmount } = withSetup(() => useHome(store, deps));
  return { h, unmount };
}
