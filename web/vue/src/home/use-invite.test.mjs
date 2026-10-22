/* Observer of `useInvite`: the invite string is a network secret, it goes away when the network changes or vanishes.
 * A fake store and `api`, `withSetup` instead of a component. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { useInvite } from "./use-invite.ts";

const SECRET = "mesh-invite2-SECRETSECRET";
// The warning text as a literal: the contract catches a typo.
const WARN_ISSUER_ONLY = "Only this computer can let them in — don't turn it off until the new member joins.";
const WARN_ISSUER_AND_ONE = "Better not to turn this computer off until the new member joins.";
const ISSUER_ONLY = { invite: SECRET, holders: 0, reach: "issuer_only" };

describe("useInvite", () => {
  it("after the request inviteText.value is the invite", async () => {
    const w = await setupRequested();
    assert.equal(w.i.inviteText.value, SECRET);
    w.unmount();
  });

  it("id_hex changed — inviteText is empty (a removed watch is caught here)", async () => {
    const w = await setupRequested();
    w.store.state = withNetwork("net-b");
    await nextTick();
    assert.equal(w.i.inviteText.value, "");
    w.unmount();
  });

  it("network vanished — inviteText is empty", async () => {
    const w = await setupRequested();
    w.store.state = { ...withNetwork("net-a"), network: null };
    await nextTick();
    assert.equal(w.i.inviteText.value, "");
    w.unmount();
  });

  it("a poll with the same id_hex does not touch the string", async () => {
    const w = await setupRequested();
    w.store.state = withNetwork("net-a");
    await nextTick();
    assert.equal(w.i.inviteText.value, SECRET);
    w.unmount();
  });

  it("the status goes away together with the string", async () => {
    const w = await setupRequested();
    assert.notEqual(w.i.status.value, "");
    w.store.state = withNetwork("net-b");
    await nextTick();
    assert.equal(w.i.status.value, "");
    w.unmount();
  });
});

describe("useInvite: warning", () => {
  it("empty before the request", () => {
    const w = setupIdle();
    assert.equal(w.i.warning.value, "");
    w.unmount();
  });

  it("after a request with reach=issuer_only — the warning text", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    assert.equal(w.i.warning.value, WARN_ISSUER_ONLY);
    w.unmount();
  });

  it("after a request with reach=spread or without the field — empty", async () => {
    for (const reply of [{ invite: SECRET, reach: "spread" }, { invite: SECRET }]) {
      const w = await setupRequested(reply);
      assert.equal(w.i.warning.value, "", JSON.stringify(reply));
      w.unmount();
    }
  });

  it("follows a new request: issuer_only → issuer_and_one → spread", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    w.node.reply = { invite: SECRET, reach: "issuer_and_one" };
    assert.equal(await w.i.request(), "copied");
    assert.equal(w.i.warning.value, WARN_ISSUER_AND_ONE);
    w.node.reply = { invite: SECRET, reach: "spread" };
    assert.equal(await w.i.request(), "copied");
    assert.equal(w.i.warning.value, "");
    w.unmount();
  });

  it("id_hex changed — goes away with the string", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    w.store.state = withNetwork("net-b");
    await nextTick();
    assert.equal(w.i.warning.value, "");
    assert.equal(w.i.inviteText.value, "");
    w.unmount();
  });

  it("network vanished — goes away", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    w.store.state = { ...withNetwork("net-a"), network: null };
    await nextTick();
    assert.equal(w.i.warning.value, "");
    w.unmount();
  });

  it("a poll with the same id_hex does not touch the warning", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    w.store.state = withNetwork("net-a");
    await nextTick();
    assert.equal(w.i.warning.value, WARN_ISSUER_ONLY);
    w.unmount();
  });

  it("after a network change a new request sets the warning again", async () => {
    const w = await setupRequested(ISSUER_ONLY);
    w.store.state = withNetwork("net-b");
    await nextTick();
    assert.equal(await w.i.request(), "copied");
    assert.equal(w.i.warning.value, WARN_ISSUER_ONLY);
    w.unmount();
  });
});

function withNetwork(idHex) {
  return { node: { status: "online" }, network: { label: "home", id_hex: idHex }, peers: [] };
}

// A fake node: `node.reply` is the response to the invite request, changes between requests.
function setupIdle(reply = { invite: SECRET }) {
  const store = createStore();
  store.state = withNetwork("net-a");
  const node = { reply };
  const deps = {
    api: async () => node.reply,
    clipboard: { writeText: async () => {} },
    setTimer: () => 1,
    clearTimer: () => {},
  };
  const { result: i, unmount } = withSetup(() => useInvite(store, deps));
  return { i, store, node, unmount };
}

async function setupRequested(reply = { invite: SECRET }) {
  const w = setupIdle(reply);
  assert.equal(await w.i.request(), "copied");
  return w;
}
