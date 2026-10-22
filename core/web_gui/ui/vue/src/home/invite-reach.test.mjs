/* Invite reach (`reachWarning`, `InviteState.warning`): the node returns `reach` —
 * "issuer_only" / "issuer_and_one" / "spread"; while the invite is held by no more than one node, a short
 * warning is shown next to the string. An old node without the field and an unknown value get no warning.
 * Texts are literals: the test catches a typo in the page text. No Vue, fake `api` and clipboard. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createInviteState, requestInvite, reachWarning } from "./invite-actions.ts";

const SECRET = "mesh-invite2-SECRETSECRET";
const WARN_ISSUER_ONLY = "Only this computer can let them in — don't turn it off until the new member joins.";
const WARN_ISSUER_AND_ONE = "Better not to turn this computer off until the new member joins.";

describe("reachWarning", () => {
  it("issuer_only — \"Only this computer can let them in…\"", () => {
    assert.equal(reachWarning("issuer_only"), WARN_ISSUER_ONLY);
  });

  it("issuer_and_one — \"Better not to turn this computer off…\"", () => {
    assert.equal(reachWarning("issuer_and_one"), WARN_ISSUER_AND_ONE);
  });

  it("spread — no warning", () => {
    assert.equal(reachWarning("spread"), "");
  });

  describe("not a string from the contract", () => {
    const noWarning = [
      ["undefined (old node without the field)", undefined],
      ["null", null],
      ["number", 1],
      ["zero", 0],
      ["boolean", true],
      ["object", { reach: "issuer_only" }],
      ["array", ["issuer_only"]],
      ["empty string", ""],
      ["foreign string", "everyone"],
      ["different case", "ISSUER_ONLY"],
      ["with spaces around", " issuer_only "],
    ];
    for (const [name, value] of noWarning) {
      it(name + " — no warning", () => {
        assert.equal(reachWarning(value), "");
      });
    }
  });
});

describe("requestInvite: reach warning", () => {
  describe("node returned reach", () => {
    it("issuer_only — warning with text, string and status as before", async () => {
      const w = setup({ response: { invite: SECRET, holders: 0, reach: "issuer_only" } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "copied");
      assert.equal(w.inv.warning, WARN_ISSUER_ONLY);
      assert.equal(w.inv.text, SECRET);
      assert.equal(w.inv.status, "✓ Invite copied — paste it into a private message");
    });

    it("issuer_and_one — warning with text", async () => {
      const w = setup({ response: { invite: SECRET, holders: 1, reach: "issuer_and_one" } });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, WARN_ISSUER_AND_ONE);
    });

    it("spread — warning is empty", async () => {
      const w = setup({ response: { invite: SECRET, holders: 2, reach: "spread" } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "copied");
      assert.equal(w.inv.warning, "");
    });
  });

  describe("node returned no clear reach", () => {
    const noWarning = [
      ["no field (old node)", { invite: SECRET }],
      ["no field, but holders present", { invite: SECRET, holders: 1 }],
      ["foreign string", { invite: SECRET, reach: "everyone" }],
      ["number instead of a string", { invite: SECRET, reach: 1 }],
      ["null", { invite: SECRET, reach: null }],
    ];
    for (const [name, response] of noWarning) {
      it(name + " — warning is empty, the invite is shown", async () => {
        const w = setup({ response });
        assert.equal(await requestInvite(w.inv, w.store, w.deps), "copied");
        assert.equal(w.inv.warning, "");
        assert.equal(w.inv.text, SECRET);
      });
    }
  });

  describe("repeat request with a new response", () => {
    it("issuer_only, then spread — the warning goes away", async () => {
      const w = setup({ response: { invite: SECRET, reach: "issuer_only" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.response = { invite: SECRET, reach: "spread" };
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, "");
    });

    it("issuer_only, then a response without reach — the warning goes away", async () => {
      const w = setup({ response: { invite: SECRET, reach: "issuer_only" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.response = { invite: SECRET };
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, "");
    });

    it("spread, then issuer_only — the warning appears", async () => {
      const w = setup({ response: { invite: SECRET, reach: "spread" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.response = { invite: SECRET, reach: "issuer_only" };
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, WARN_ISSUER_ONLY);
    });

    it("issuer_only, then issuer_and_one — the text is replaced, not concatenated", async () => {
      const w = setup({ response: { invite: SECRET, reach: "issuer_only" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.response = { invite: SECRET, reach: "issuer_and_one" };
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, WARN_ISSUER_AND_ONE);
    });
  });

  describe("repeat request failed", () => {
    it("409 — string and warning unchanged, status \"isn't ready yet\"", async () => {
      const w = setup({ response: { invite: SECRET, reach: "issuer_only" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.fail = new ApiError(409, "busy", "busy");
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.equal(w.inv.text, SECRET);
      assert.equal(w.inv.warning, WARN_ISSUER_ONLY);
      assert.match(w.inv.status, /isn't ready yet/);
    });

    it("empty response — string and warning unchanged", async () => {
      const w = setup({ response: { invite: SECRET, reach: "issuer_and_one" } });
      await requestInvite(w.inv, w.store, w.deps);
      w.node.response = {};
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.equal(w.inv.text, SECRET);
      assert.equal(w.inv.warning, WARN_ISSUER_AND_ONE);
    });

    it("response without an invite, but with reach — the warning is not set", async () => {
      const w = setup({ response: { reach: "issuer_only" } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.equal(w.inv.warning, "");
    });

    it("the very first request refused — no warning", async () => {
      const w = setup({ fail: new ApiError(409, "busy", "busy") });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.warning, "");
    });
  });

  describe("clipboard unavailable", () => {
    it("no clipboard — \"manual\", the warning is still shown", async () => {
      const w = setup({ clipboard: null, response: { invite: SECRET, reach: "issuer_only" } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "manual");
      assert.equal(w.inv.warning, WARN_ISSUER_ONLY);
      assert.equal(w.inv.text, SECRET);
    });

    it("writeText rejected — \"manual\", the warning is shown", async () => {
      const denied = { writeText: async () => { throw new Error("denied"); } };
      const w = setup({ clipboard: denied, response: { invite: SECRET, reach: "issuer_and_one" } });
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "manual");
      assert.equal(w.inv.warning, WARN_ISSUER_AND_ONE);
    });

    it("clipboard write still in progress — the warning is already shown with the string", async () => {
      const slow = { writeText: () => new Promise(() => {}) };
      const w = setup({ clipboard: slow, response: { invite: SECRET, reach: "issuer_only" } });
      void requestInvite(w.inv, w.store, w.deps);
      await tick();
      assert.equal(w.inv.text, SECRET);
      assert.equal(w.inv.warning, WARN_ISSUER_ONLY);
    });
  });

  describe("network changed in flight", () => {
    it("\"stale\" — the warning is not set", async () => {
      const w = setup({ hold: true, response: { invite: SECRET, reach: "issuer_only" } });
      const pending = requestInvite(w.inv, w.store, w.deps);
      await tick();
      w.store.state = { ...w.store.state, network: { label: "other", id_hex: "net-b" } };
      w.release();
      assert.equal(await pending, "stale");
      assert.equal(w.inv.warning, "");
      assert.equal(w.inv.text, "");
    });
  });

  describe("secret", () => {
    it("neither the warning nor the store has an invite string for any reach", async () => {
      for (const reach of ["issuer_only", "issuer_and_one", "spread"]) {
        const w = setup({ response: { invite: SECRET, reach } });
        await requestInvite(w.inv, w.store, w.deps);
        assert.doesNotMatch(w.inv.warning, /SECRETSECRET/, reach);
        assert.doesNotMatch(JSON.stringify(w.store), /SECRETSECRET/, reach);
      }
    });
  });
});

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

// Preconditions: node response (`response`), refusal (`fail`), clipboard (writing by default), `hold` — the response waits for
// `release()`. `node.response` / `node.fail` change between requests of one test.
function setup({ clipboard, fail = null, hold = false, response = { invite: SECRET } } = {}) {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "net-a" }, peers: [] };
  let release = () => {};
  const gate = new Promise((resolve) => { release = resolve; });
  const node = { fail, response };
  const api = async () => {
    if (hold) await gate;
    if (node.fail) throw node.fail;
    return node.response;
  };
  const buffer = clipboard === undefined ? { writeText: async () => {} } : clipboard;
  return { inv: createInviteState(), store, release, node, deps: { api, clipboard: buffer } };
}
