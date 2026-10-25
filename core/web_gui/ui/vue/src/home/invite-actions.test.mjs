/* Invite of the Home tab without Vue (`invite-actions.ts`). The invite string is a network secret: it lives
 * only in `inv.text` and does not reach the store or storages. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createInviteState, requestInvite, resetInviteIfNetChanged } from "./invite-actions.ts";

const SECRET = "mesh-invite2-SECRETSECRET";
// The warning text as a literal (the full set of reach cases is in `invite-reach.test.mjs`).
const WARN_ISSUER_ONLY = "Only this computer can let them in — don't turn it off until the new member joins.";

describe("requestInvite", () => {
  describe("clipboard available", () => {
    it("\"copied\", text is the invite, netId is the network's id_hex at request time", async () => {
      const w = setup();
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "copied");
      assert.deepEqual(w.calls, [["GET", "/api/network/invite"]]);
      assert.equal(w.inv.text, SECRET);
      assert.equal(w.inv.netId, "net-a");
      assert.deepEqual(w.written, [SECRET]);
      assert.equal(w.inv.status, "✓ Invite copied — paste it into a private message");
    });
  });

  describe("no clipboard", () => {
    it("\"manual\", the string is visible, the status asks to copy manually", async () => {
      const w = setup({ clipboard: null });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "manual");
      assert.equal(w.inv.text, SECRET);
      assert.match(w.inv.status, /^Clipboard unavailable — select the invite below/);
    });

    it("writeText rejected — same \"manual\"", async () => {
      const w = setup({ clipboard: { writeText: async () => { throw new Error("denied"); } } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "manual");
      assert.equal(w.inv.text, SECRET);
      assert.match(w.inv.status, /manually/);
    });
  });

  describe("node refused", () => {
    it("409 — \"failed\", \"isn't ready yet\", no string", async () => {
      const w = setup({ fail: new ApiError(409, "busy", "busy") });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.match(w.inv.status, /isn't ready yet/);
      assert.equal(w.inv.text, "");
    });
  });

  describe("response without an invite", () => {
    it("empty response — \"failed\" and the text \"isn't ready yet\"", async () => {
      const w = setup({ response: {} });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.match(w.inv.status, /^The invite isn't ready yet/);
    });
  });

  describe("network changed in flight", () => {
    it("\"stale\", text is empty, clipboard untouched", async () => {
      const w = setup({ hold: true });
      const pending = requestInvite(w.inv, w.store, w.deps);
      await tick();
      w.store.state = { ...w.store.state, network: { label: "other", id_hex: "net-b" } };
      w.release();
      assert.equal(await pending, "stale");
      assert.equal(w.inv.text, "");
      assert.match(w.inv.status, /^The network changed — click "Invite a member" again\.$/);
      assert.deepEqual(w.written, []);
    });

    it("id_hex is taken before the request: netId is the old network's, not the new one's", async () => {
      const w = setup({ hold: true });
      const pending = requestInvite(w.inv, w.store, w.deps);
      await tick();
      w.store.state = { ...w.store.state, network: { label: "other", id_hex: "net-b" } };
      w.release();
      await pending;
      assert.notEqual(w.inv.netId, "net-b", "an invite of the old network is not attributed to the new one");
    });
  });

  describe("repeat in flight", () => {
    it("\"skipped\", one request", async () => {
      const w = setup({ hold: true });
      const first = requestInvite(w.inv, w.store, w.deps);
      await tick();
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "skipped");
      w.release();
      await first;
      assert.equal(w.calls.length, 1);
    });
  });

  describe("secret", () => {
    it("after any outcome the store has no invite string", async () => {
      for (const opts of [{}, { clipboard: null }, { fail: new ApiError(409, "busy", "busy") }]) {
        const w = setup(opts);
        await requestInvite(w.inv, w.store, w.deps);
        assert.doesNotMatch(JSON.stringify(w.store), /SECRETSECRET/);
      }
    });
  });
});

describe("createInviteState", () => {
  it("warning is empty, other fields as before", () => {
    assert.equal(createInviteState().warning, "");
    assert.deepEqual(createInviteState(), { text: "", qr: "", netId: null, busy: false, status: "", warning: "" });
  });
});

describe("resetInviteIfNetChanged", () => {
  const shown = () => ({ text: SECRET, netId: "net-a", busy: false, status: "copied", warning: WARN_ISSUER_ONLY });

  it("same network — string and warning intact", () => {
    const inv = shown();
    resetInviteIfNetChanged(inv, "net-a");
    assert.equal(inv.text, SECRET);
    assert.equal(inv.status, "copied");
    assert.equal(inv.warning, WARN_ISSUER_ONLY);
  });

  it("different id_hex — text, netId, status, warning reset", () => {
    const inv = shown();
    resetInviteIfNetChanged(inv, "net-b");
    assert.deepEqual(inv, { text: "", qr: "", netId: null, busy: false, status: "", warning: "" });
  });

  it("no network (null) — reset, the warning goes away", () => {
    const inv = shown();
    resetInviteIfNetChanged(inv, null);
    assert.equal(inv.text, "");
    assert.equal(inv.netId, null);
    assert.equal(inv.warning, "");
  });

  it("network unknown (undefined) — the warning goes away", () => {
    const inv = shown();
    resetInviteIfNetChanged(inv, undefined);
    assert.equal(inv.warning, "");
  });

  it("invite was never requested — empty stays empty", () => {
    const inv = createInviteState();
    resetInviteIfNetChanged(inv, "net-a");
    assert.deepEqual(inv, createInviteState());
  });
});

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

// Preconditions: clipboard (writing by default), node refusal, `hold` — the response waits for `release()`.
function setup({ clipboard, fail = null, hold = false, response = { invite: SECRET } } = {}) {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "net-a" }, peers: [] };
  const written = [];
  const calls = [];
  let release = () => {};
  const gate = new Promise((resolve) => { release = resolve; });
  const api = async (method, path) => {
    calls.push([method, path]);
    if (hold) await gate;
    if (fail) throw fail;
    return response;
  };
  const buffer = clipboard === undefined ? { writeText: async (t) => { written.push(t); } } : clipboard;
  return { inv: createInviteState(), store, calls, written, release, deps: { api, clipboard: buffer } };
}
