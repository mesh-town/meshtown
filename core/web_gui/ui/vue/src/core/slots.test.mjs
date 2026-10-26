/* `slots.ts`: slot addresses, which slots are visible, how a network is named. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { slotPath, isSlotId, visibleSlots, peersSlots, brokenSlots, isRefused, isMulti, netName, refusedFlag } from "./slots.ts";

const B = "k7q2xa";
const C = "m3n4pq";

describe("slotPath", () => {
  describe("main slot", () => {
    it("members/leave goes to /api/members/leave", () => {
      assert.equal(slotPath("main", "members/leave"), "/api/members/leave");
    });
    it("config goes to /api/config", () => {
      assert.equal(slotPath("main", "config"), "/api/config");
    });
    it("a query tail is kept as is", () => {
      assert.equal(slotPath("main", "network/invite?for=a"), "/api/network/invite?for=a");
    });
  });

  describe("another slot", () => {
    it("members/leave goes to /api/net/<slot>/members/leave", () => {
      assert.equal(slotPath(B, "members/leave"), "/api/net/k7q2xa/members/leave");
    });
    it("a query tail with ?for=a is not escaped again", () => {
      assert.equal(slotPath(B, "network/invite?for=a"), "/api/net/k7q2xa/network/invite?for=a");
    });
    it("an encoded tail stays byte for byte", () => {
      assert.equal(slotPath(B, "domains?name=a%20b"), "/api/net/k7q2xa/domains?name=a%20b");
    });
  });
});

describe("isSlotId", () => {
  for (const id of ["main", B]) {
    it(`${id} is a slot id`, () => {
      assert.equal(isSlotId(id), true);
    });
  }
  for (const id of ["K7Q2XA", "k7q2x1", "k7q2xab", "k7q2x", "", "../x"]) {
    it(`"${id}" is not a slot id`, () => {
      assert.equal(isSlotId(id), false);
    });
  }
});

describe("visibleSlots", () => {
  describe("main without a key, another network with a key", () => {
    it("only the other network is visible", () => {
      const { nets, slots } = setup([brief("main"), brief(B, { id_hex: "3f2a00" })]);
      assert.deepEqual(visibleSlots(nets, slots), [B]);
    });
  });

  describe("another network with a key and label null", () => {
    it("is visible: the key, not the label, is the sign", () => {
      const { nets, slots } = setup([brief("main"), brief(B, { label: null, id_hex: "3f2a00" })]);
      assert.deepEqual(visibleSlots(nets, slots), [B]);
    });
  });

  describe("a label without a key", () => {
    it("is not enough to be visible", () => {
      const { nets, slots } = setup([
        brief("main", { label: "home", id_hex: null }),
        brief(B, { label: "friends", id_hex: "3f2a00" }),
      ]);
      assert.deepEqual(visibleSlots(nets, slots), [B]);
    });
  });

  // Contract: when nothing holds a key, visibleSlots itself answers ["main"] (the setup screen).
  describe("no network has a key", () => {
    it("falls back to the main slot", () => {
      const { nets, slots } = setup([brief("main"), brief(B)]);
      assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    });
    it("an empty list falls back to the main slot", () => {
      assert.deepEqual(visibleSlots([], {}), ["main"]);
    });
  });

  describe("a network that is joining", () => {
    it("is visible without a key", () => {
      const { nets, slots } = setup([
        brief("main", { id_hex: "aaaa11" }),
        brief(B, { joining: true }),
      ]);
      assert.deepEqual(visibleSlots(nets, slots), ["main", B]);
    });
  });

  describe("a network whose state is null", () => {
    it("is hidden", () => {
      const { nets, slots } = setup(
        [brief("main", { id_hex: "aaaa11" }), brief(B, { id_hex: "3f2a00" })],
        { [B]: { state: null } },
      );
      assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    });
    it("a joining one with no state is hidden too", () => {
      const { nets, slots } = setup(
        [brief("main", { id_hex: "aaaa11" }), brief(B, { joining: true })],
        { [B]: { state: null } },
      );
      assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    });
  });

  describe("a brief with no slot slice", () => {
    it("is hidden", () => {
      const { nets, slots } = setup([brief("main", { id_hex: "aaaa11" }), brief(B, { id_hex: "3f2a00" })]);
      delete slots[B];
      assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    });
  });

  describe("a slice that has no brief", () => {
    it("is not listed", () => {
      const { nets, slots } = setup([brief("main", { id_hex: "aaaa11" })]);
      Object.assign(slots, setup([brief(B, { id_hex: "3f2a00" })]).slots);
      assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    });
  });

  describe("order", () => {
    it("follows the order of nets, not of the slices", () => {
      const { nets, slots } = setup([
        brief(C, { id_hex: "9c0100" }),
        brief("main", { id_hex: "aaaa11" }),
        brief(B, { id_hex: "3f2a00" }),
      ]);
      assert.deepEqual(visibleSlots(nets, slots), [C, "main", B]);
    });
  });
});

describe("isMulti", () => {
  it("one visible network is not multi", () => {
    const { nets, slots } = setup([brief("main", { id_hex: "aaaa11" }), brief(B)]);
    assert.equal(isMulti({ nets, slots }), false);
  });
  it("no visible network (fallback to main) is not multi", () => {
    assert.equal(isMulti({ nets: [], slots: {} }), false);
  });
  it("two visible networks are multi", () => {
    const { nets, slots } = setup([brief("main", { id_hex: "aaaa11" }), brief(B, { id_hex: "3f2a00" })]);
    assert.equal(isMulti({ nets, slots }), true);
  });
});

describe("netName", () => {
  describe("a label", () => {
    it("is the name", () => {
      const nets = [brief("main", { label: "home", id_hex: "aaaa11" }), brief(B, { label: "friends", id_hex: "3f2a00" })];
      assert.equal(netName(nets, B), "friends");
    });
  });

  describe("label null, id_hex present", () => {
    it("is Network <first 4 chars of id_hex>", () => {
      const nets = [brief(B, { label: null, id_hex: "3f2a9c01deadbeef" })];
      assert.equal(netName(nets, B), "Network 3f2a");
    });
    it("takes exactly four chars: a shorter or longer cut would differ", () => {
      const nets = [brief(B, { label: null, id_hex: "3f2a9c" })];
      assert.equal(netName(nets, B), "Network 3f2a");
    });
  });

  describe("label and id_hex both null", () => {
    it("is the slot id", () => {
      assert.equal(netName([brief(B)], B), B);
    });
  });

  describe("a slot that is joining and holds no key yet", () => {
    it("without a label is New network, not the slot id", () => {
      assert.equal(netName([brief(B, { joining: true })], B), "New network");
    });
    it("with a label is the label", () => {
      assert.equal(netName([brief(B, { joining: true, label: "friends" })], B), "friends");
    });
    it("two of them without labels get no suffix: there is no key to take it from", () => {
      const nets = [brief(B, { joining: true }), brief(C, { joining: true })];
      assert.equal(netName(nets, B), "New network");
      assert.equal(netName(nets, C), "New network");
    });
  });

  describe("a slot that is not joining and holds no key", () => {
    it("without a label stays the slot id", () => {
      assert.equal(netName([brief(B, { joining: false })], B), B);
    });
  });

  describe("an unknown id", () => {
    it("is the id itself", () => {
      assert.equal(netName([brief(B, { id_hex: "3f2a00" })], C), C);
    });
  });

  describe("two visible networks with the same label", () => {
    it("both get a suffix of the first 4 chars of their id_hex", () => {
      const nets = [
        brief(B, { label: "friends", id_hex: "3f2a00ff" }),
        brief(C, { label: "friends", id_hex: "9c0123ab" }),
      ];
      assert.equal(netName(nets, B), "friends · 3f2a");
      assert.equal(netName(nets, C), "friends · 9c01");
    });
    it("case does not matter when comparing", () => {
      const nets = [
        brief(B, { label: "Friends", id_hex: "3f2a00ff" }),
        brief(C, { label: "friends", id_hex: "9c0123ab" }),
      ];
      assert.equal(netName(nets, B), "Friends · 3f2a");
      assert.equal(netName(nets, C), "friends · 9c01");
    });
    it("spaces do not matter when comparing", () => {
      const nets = [
        brief(B, { label: "my net", id_hex: "3f2a00ff" }),
        brief(C, { label: "mynet", id_hex: "9c0123ab" }),
      ];
      assert.equal(netName(nets, B), "my net · 3f2a");
      assert.equal(netName(nets, C), "mynet · 9c01");
    });
  });

  describe("the same label on a visible and a hidden network", () => {
    it("the visible one gets no suffix", () => {
      // hidden: no key and not joining
      const nets = [brief(B, { label: "friends", id_hex: "3f2a00ff" }), brief(C, { label: "friends", id_hex: null })];
      assert.equal(netName(nets, B), "friends");
    });
  });

  describe("different labels", () => {
    it("get no suffix", () => {
      const nets = [
        brief(B, { label: "friends", id_hex: "3f2a00ff" }),
        brief(C, { label: "work", id_hex: "9c0123ab" }),
      ];
      assert.equal(netName(nets, B), "friends");
      assert.equal(netName(nets, C), "work");
    });
  });
});

describe("a refused join", () => {
  const refusedState = { node: { invite_join: { state: "refused", message: "No." } } };
  const world = () => setup([brief("main", { id_hex: "aa" }), brief(B)], { [B]: { state: refusedState } });

  it("is not a visible slot, but stays among the tabs of the Members page", () => {
    const { nets, slots } = world();
    assert.deepEqual(visibleSlots(nets, slots), ["main"]);
    assert.deepEqual(peersSlots(nets, slots), ["main", B]);
    assert.equal(isRefused(nets, slots, B), true);
  });

  it("is gone with the slot's refusal", () => {
    const { nets, slots } = setup([brief("main", { id_hex: "aa" }), brief(B)], { [B]: { state: { node: {} } } });
    assert.deepEqual(peersSlots(nets, slots), ["main"]);
  });

  it("a slot that is joining again is not refused", () => {
    const { nets, slots } = setup([brief("main", { id_hex: "aa" }), brief(B, { joining: true })], { [B]: { state: refusedState } });
    assert.equal(isRefused(nets, slots, B), false);
  });
});

function brief(id, over = {}) {
  return {
    id, label: null, id_hex: null, status: "running", exit_enabled: false, proxy: id === "main",
    route_state: "direct", exits: [], joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0, admission_refused: false, ...over,
  };
}

/** Briefs plus a slice with a (placeholder) state for each; `sliceOver[id]` overrides a slice. */
function setup(nets, sliceOver = {}) {
  const slots = {};
  for (const b of nets) {
    slots[b.id] = {
      id: b.id, brief: b, state: {}, members: null, config: null, configRev: null,
      configMeta: null, bootId: null, ...(sliceOver[b.id] ?? {}),
    };
  }
  return { nets, slots };
}

describe("refusedFlag", () => {
  const store = { nets: [{ id: "main", admission_refused: true }, { id: B, admission_refused: false }] };
  it("is true for a slot the members refused", () => {
    assert.equal(refusedFlag(store, "main"), true);
  });
  it("is false for a slot that is not refused", () => {
    assert.equal(refusedFlag(store, B), false);
  });
  it("is false for an unknown id", () => {
    assert.equal(refusedFlag(store, C), false);
  });
  it("is false when there are no nets", () => {
    assert.equal(refusedFlag({ nets: [] }, "main"), false);
  });
});

describe("a broken slot", () => {
  const nets = () => [brief("main", { id_hex: "aaaa11" }), brief(B, { id_hex: null, label: null, status: "broken" })];
  const noState = { [B]: { state: null } };

  it("peersSlots lists a broken slot, visibleSlots and isMulti do not", () => {
    const { nets: n, slots } = setup(nets(), noState);
    assert.deepEqual(peersSlots(n, slots), ["main", B]);
    assert.deepEqual(visibleSlots(n, slots), ["main"]);
    assert.equal(isMulti({ nets: n, slots, config: null }), false);
  });

  it("brokenSlots skips main and live slots", () => {
    const list = [brief("main", { status: "broken" }), brief(B, { status: "broken" }), brief(C, { status: "online", id_hex: "3f2a00" })];
    assert.deepEqual(brokenSlots(list), [B]);
  });
});
