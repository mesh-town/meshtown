/* `acceptOverview`: laying slots into the store; the legacy synthetic overview; leaving a network. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { toRaw, watch } from "vue";
import { acceptOverview, legacyOverview, briefFromState, putSlotMembers, dropSlot } from "./overview.ts";
import { createStore } from "./store.ts";
import { baseState, SELF_FULL } from "../testing/section.mjs";
import { membersReply } from "../testing/members-fixtures.mjs";
import { B, C, KEYLESS, MAIN, brief, keylessMainAndB, overviewOf, plain, setup, stateOf, twoSpecs } from "./overview-test-support.mjs";

describe("acceptOverview", () => {
  describe("one network", () => {
    it("lead is main and store.state is the very state of the main slice", () => {
      const w = setup([{ id: MAIN }]);
      assert.equal(w.store.lead, MAIN);
      assert.equal(toRaw(w.store.state), w.ov.slots[0].state);
      assert.equal(w.store.state, w.store.slots[MAIN].state);
    });
    it("store.members and store.nets come from the overview", () => {
      const w = setup([{ id: MAIN }]);
      assert.equal(toRaw(w.store.members), w.ov.slots[0].members);
      assert.deepEqual(plain(w.store.nets), w.ov.nets);
    });
    it("the main slice never holds a config: the document of main is store.config", () => {
      const w = setup([{ id: MAIN }]);
      assert.equal(w.store.slots[MAIN].config, null);
      assert.equal(w.store.slots[MAIN].configRev, null);
    });
    it("the slice carries the brief of its network", () => {
      const w = setup([{ id: MAIN }]);
      assert.deepEqual(plain(w.store.slots[MAIN].brief), w.ov.nets[0]);
    });
  });

  describe("main without a key and a network B with a key", () => {
    it("lead is B", () => {
      assert.equal(setup(keylessMainAndB()).store.lead, B);
    });
    it("store.state is B's state with socks, web and log of main", () => {
      const { store, ov } = setup(keylessMainAndB());
      const [mainState, bState] = [ov.slots[0].state, ov.slots[1].state];
      assert.deepEqual(plain(store.state.socks), mainState.socks);
      assert.deepEqual(plain(store.state.web), mainState.web);
      assert.deepEqual(plain(store.state.log), mainState.log, "the log summary is the main process's");
      assert.deepEqual(plain(store.state.node), bState.node, "the node block is B's own");
      assert.deepEqual(plain(store.state.peers), bState.peers);
    });
    it("store.members is B's member list", () => {
      const { store, ov } = setup(keylessMainAndB());
      assert.equal(toRaw(store.members), ov.slots[1].members);
    });
    it("without a main state, store.state is B's state as it is", () => {
      const { store, ov } = setup([{ id: MAIN, brief: KEYLESS, state: null }, { id: B }]);
      assert.equal(toRaw(store.state), ov.slots[1].state);
    });
  });

  describe("no network holds a key", () => {
    it("lead is main", () => {
      const w = setup([{ id: MAIN, brief: KEYLESS }, { id: B, brief: KEYLESS }]);
      assert.equal(w.store.lead, MAIN);
    });
  });

  describe("B has no key but is being joined", () => {
    it("B is visible and is the lead", () => {
      const w = setup([{ id: MAIN, brief: KEYLESS }, { id: B, brief: { ...KEYLESS, joining: true } }]);
      assert.equal(w.store.lead, B);
    });
  });

  describe("B has a key but its state is null", () => {
    it("B is hidden (main stays the lead) yet its slice exists", () => {
      const w = setup([{ id: MAIN }, { id: B, state: null }]);
      assert.equal(w.store.lead, MAIN);
      assert.ok(w.store.slots[B], "a hidden slot must keep its slice");
      assert.equal(w.store.slots[B].state, null);
    });
  });

  describe("the lead has no state", () => {
    it("store.state keeps the previous snapshot", () => {
      const w = setup([{ id: MAIN }]);
      const before = w.store.state;
      acceptOverview(w.store, overviewOf([{ id: MAIN, state: null }]));
      assert.equal(w.store.state, before);
    });
  });

  describe("the same overview again", () => {
    it("the slice of B is the same object and keeps its config", () => {
      const w = setup(twoSpecs());
      const slice = w.store.slots[B];
      slice.config = { route: {} };
      acceptOverview(w.store, overviewOf(twoSpecs()));
      assert.equal(w.store.slots[B], slice);
      assert.deepEqual(plain(w.store.slots[B].config), { route: {} });
    });
    it("the state of the slice is replaced by the new one", () => {
      const w = setup(twoSpecs());
      const next = overviewOf(twoSpecs());
      acceptOverview(w.store, next);
      assert.equal(toRaw(w.store.slots[B].state), next.slots[1].state);
    });
  });

  describe("B vanishes from the overview", () => {
    it("its slice is gone", () => {
      const w = setup(twoSpecs());
      acceptOverview(w.store, overviewOf([{ id: MAIN }]));
      assert.deepEqual(Object.keys(w.store.slots), [MAIN]);
    });
  });

  describe("a slot id twice", () => {
    it("the first entry wins", () => {
      const ov = overviewOf([{ id: MAIN }, { id: B, state: { boot_id: "first" } }]);
      ov.slots.push({ slot: B, state: baseState({ boot_id: "second" }), members: null });
      const store = createStore();
      acceptOverview(store, ov);
      assert.equal(store.slots[B].state.boot_id, "first");
    });
  });

  describe("restart_slots", () => {
    it("keeps the known ids in the order of the node", () => {
      const w = setup(twoSpecs(), { restart: [B, MAIN, "zzzzzz"] });
      assert.deepEqual(plain(w.store.restartSlots), [B, MAIN]);
    });
  });

  describe("order of writes", () => {
    it("a synchronous observer of store.state already sees the new lead and slots", () => {
      const store = createStore();
      acceptOverview(store, overviewOf([{ id: MAIN }]));
      const seen = [];
      watch(() => store.state, () => seen.push({ lead: store.lead, slots: Object.keys(store.slots) }), { flush: "sync" });
      acceptOverview(store, overviewOf(keylessMainAndB()));
      assert.ok(seen.length > 0, "store.state was not rewritten");
      for (const s of seen) assert.deepEqual(s, { lead: B, slots: [MAIN, B] });
    });
    it("no observer ever sees store.slots empty", () => {
      const store = createStore();
      acceptOverview(store, overviewOf([{ id: MAIN }]));
      const sizes = [];
      watch(() => Object.keys(store.slots).length, (n) => sizes.push(n), { flush: "sync" });
      acceptOverview(store, overviewOf(twoSpecs()));
      acceptOverview(store, overviewOf([{ id: MAIN }]));
      assert.ok(sizes.length > 0 && sizes.every((n) => n > 0), "sizes seen: " + sizes.join(","));
    });
    it("no observer sees store.nets empty or stale members", () => {
      const store = createStore();
      acceptOverview(store, overviewOf([{ id: MAIN }]));
      const seen = [];
      watch(() => store.members, () => seen.push({ nets: store.nets.length, lead: store.lead }), { flush: "sync" });
      acceptOverview(store, overviewOf(keylessMainAndB()));
      for (const s of seen) assert.deepEqual(s, { nets: 2, lead: B });
    });
  });
});

describe("legacyOverview", () => {
  describe("a state with restart_pending", () => {
    it("restart_slots is [main]", () => {
      const ov = legacyOverview(stateOf(MAIN, { restart_pending: ["socks.ifaces"] }), null);
      assert.deepEqual(ov.restart_slots, [MAIN]);
    });
  });

  describe("a state without restart_pending or with an empty one", () => {
    it("restart_slots is empty", () => {
      assert.deepEqual(legacyOverview(stateOf(MAIN), null).restart_slots, []);
      assert.deepEqual(legacyOverview(stateOf(MAIN, { restart_pending: [] }), null).restart_slots, []);
    });
  });

  describe("any state", () => {
    it("is one synthetic main slot with the state and the member list as given", () => {
      const state = stateOf(MAIN);
      const members = membersReply();
      const ov = legacyOverview(state, members);
      assert.equal(ov.slots.length, 1);
      assert.equal(ov.slots[0].slot, MAIN);
      assert.equal(ov.slots[0].state, state);
      assert.equal(ov.slots[0].members, members);
    });
    it("the brief takes label and id_hex from the network block", () => {
      const state = stateOf(MAIN, { network: { label: "friends", id_hex: "3f2a01", masked: "m" } });
      const [n] = legacyOverview(state, null).nets;
      assert.deepEqual([n.id, n.label, n.id_hex], [MAIN, "friends", "3f2a01"]);
    });
    it("a state without a network gives label and id_hex null", () => {
      const [n] = legacyOverview(stateOf(MAIN, { network: null }), null).nets;
      assert.deepEqual([n.label, n.id_hex], [null, null]);
    });
  });
});

describe("briefFromState", () => {
  it("counts peers except 'You'", () => {
    const n = briefFromState(MAIN, stateOf(MAIN), null);
    assert.deepEqual([n.peers_online, n.peers_total], [1, 2]);
  });
  it("only main is the proxy", () => {
    assert.equal(briefFromState(MAIN, stateOf(MAIN), null).proxy, true);
    assert.equal(briefFromState(B, stateOf(B), null).proxy, false);
  });
  it("joining while the invite waits", () => {
    const waiting = stateOf(B, { node: { ...stateOf(B).node, invite_join: { state: "waiting", message: "", label: null, expires_unix: 1 } } });
    assert.equal(briefFromState(B, waiting, null).joining, true);
    assert.equal(briefFromState(B, stateOf(B), null).joining, false);
  });
  it("a null state is a broken network with zeros", () => {
    const n = briefFromState(B, null, null);
    assert.equal(n.status, "broken");
    assert.equal(n.route_state, "unavailable");
    assert.deepEqual([n.peers_online, n.peers_total, n.members_voting, n.joining, n.me_excluded], [0, 0, 0, false, false]);
  });
});

describe("briefFromState: admission refusal and waiting", () => {
  const withMembers = (over) => stateOf(B, { members: { count: 2, gossip_neighbors: 0, ...over } });
  it("admission_refused comes from state.members", () => {
    assert.equal(briefFromState(B, withMembers({ admission_refused: true }), null).admission_refused, true);
    assert.equal(briefFromState(B, withMembers({ admission_refused: false }), null).admission_refused, false);
    assert.equal(briefFromState(B, stateOf(B), null).admission_refused, false);
  });
  it("a null state: not refused", () => {
    assert.equal(briefFromState(B, null, null).admission_refused, false);
  });
  it("me_pending: the member list when it is asked, else the state", () => {
    const state = withMembers({ me_pending: true });
    assert.equal(briefFromState(B, state, null).me_pending, true);
    assert.equal(briefFromState(B, state, { me_pending: false, members: [] }).me_pending, false);
    assert.equal(briefFromState(B, withMembers({}), { me_pending: true, members: [] }).me_pending, true);
  });
});

describe("putSlotMembers", () => {
  describe("the lead slot", () => {
    it("updates the slice and store.members", () => {
      const w = setup(twoSpecs());
      putSlotMembers(w.store, MAIN, membersReply({ need: 3 }));
      assert.equal(w.store.slots[MAIN].members.need, 3);
      assert.equal(w.store.members.need, 3);
    });
  });

  describe("a slot that is not the lead", () => {
    it("updates only the slice", () => {
      const w = setup(twoSpecs());
      const before = w.store.members;
      putSlotMembers(w.store, B, membersReply({ need: 5 }));
      assert.equal(w.store.slots[B].members.need, 5);
      assert.equal(w.store.members, before);
    });
  });

  describe("an unknown slot", () => {
    it("changes nothing", () => {
      const w = setup(twoSpecs());
      putSlotMembers(w.store, C, membersReply());
      assert.deepEqual(Object.keys(w.store.slots), [MAIN, B]);
    });
  });
});

describe("dropSlot", () => {
  describe("B leaves from a tab of B", () => {
    it("the slice is gone, the lead and the tab are main", () => {
      const w = setup(twoSpecs());
      w.store.peersTab = B;
      dropSlot(w.store, B, [brief(MAIN)]);
      assert.equal(w.store.slots[B], undefined);
      assert.equal(w.store.lead, MAIN);
      assert.equal(w.store.peersTab, MAIN);
      assert.deepEqual(plain(w.store.nets).map((n) => n.id), [MAIN]);
    });
    it("store.state and store.members become the ones of main", () => {
      const w = setup(keylessMainAndB());
      dropSlot(w.store, B, [brief(MAIN)]);
      assert.equal(toRaw(w.store.state), w.ov.slots[0].state);
      assert.equal(toRaw(w.store.members), w.ov.slots[0].members);
    });
  });

  describe("the reply has no nets (an old node)", () => {
    it("store.nets stays as it was", () => {
      const w = setup(twoSpecs());
      dropSlot(w.store, B, []);
      assert.deepEqual(plain(w.store.nets).map((n) => n.id), [MAIN, B]);
    });
  });

  describe("main leaves while B stays", () => {
    it("the main slice has no member list and B is the lead", () => {
      const w = setup(twoSpecs());
      dropSlot(w.store, MAIN, [brief(B)]);
      assert.equal(w.store.slots[MAIN].members, null);
      assert.equal(w.store.lead, B);
    });
  });
});
