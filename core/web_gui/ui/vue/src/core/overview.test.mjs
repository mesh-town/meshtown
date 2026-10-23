/* `overview.ts`: the soft parse of `GET /api/overview`, laying slots into the store (`acceptOverview`), the list of slots whose
 * config must be (re)read, the legacy synthetic overview, leaving a network. Plus `eachSlot` of `save-config.ts`: a write to several networks. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { toRaw, watch } from "vue";
import { parseOverview, acceptOverview, legacyOverview, briefFromState, putSlotMembers, dropSlot } from "./overview.ts";
import { eachSlot, slotFailureText } from "./save-config.ts";
import { createStore } from "./store.ts";
import { baseState, SELF_FULL } from "../testing/section.mjs";
import { membersReply } from "../testing/members-fixtures.mjs";

const MAIN = "main";
const B = "k7q2xa";
const C = "m3n4pq";
const KEYLESS = { label: null, id_hex: null };

describe("parseOverview", () => {
  describe("a well-formed reply", () => {
    it("keeps api, nets, slots and restart_slots", () => {
      const ov = parseOverview(rawOverview({ restart_slots: [MAIN] }));
      assert.equal(ov.api, 2);
      assert.deepEqual(ov.nets.map((n) => n.id), [MAIN, B]);
      assert.deepEqual(ov.slots.map((s) => s.slot), [MAIN, B]);
      assert.deepEqual(ov.restart_slots, [MAIN]);
    });
    it("a slot has its state and a parsed member list", () => {
      const slot = parseOverview(rawOverview()).slots[1];
      assert.equal(slot.state.node.id, "node-" + B);
      assert.equal(slot.members.me, SELF_FULL);
      assert.equal(slot.members.members.length, 3);
    });
  });

  describe("extra fields", () => {
    it("are dropped from the overview, the brief and the slot; the rest is parsed", () => {
      const raw = rawOverview({ extra: 1 });
      raw.nets[0].colour = "red";
      raw.slots[0].tag = "x";
      const ov = parseOverview(raw);
      assert.deepEqual(Object.keys(ov).sort(), ["api", "nets", "restart_slots", "slots"]);
      assert.deepEqual(Object.keys(ov.nets[0]).sort(), Object.keys(brief(MAIN)).sort());
      assert.deepEqual(Object.keys(ov.slots[0]).sort(), ["members", "slot", "state"]);
    });
  });

  describe("api", () => {
    it("3 is not an error", () => {
      assert.equal(parseOverview(rawOverview({ api: 3 })).api, 3);
    });
    it("not a number reads as 0", () => {
      assert.equal(parseOverview(rawOverview({ api: "2" })).api, 0);
    });
  });

  describe("a status or route_state the page does not know", () => {
    it("route_state of another shape reads as unavailable", () => {
      const raw = rawOverview();
      raw.nets[0].route_state = "teleport";
      assert.equal(parseOverview(raw).nets[0].route_state, "unavailable");
    });
    it("status weird reads as unavailable", () => {
      const raw = rawOverview();
      raw.nets[0].status = "weird";
      assert.equal(parseOverview(raw).nets[0].status, "unavailable");
    });
    it("status broken is kept", () => {
      const raw = rawOverview();
      raw.nets[0].status = "broken";
      assert.equal(parseOverview(raw).nets[0].status, "broken");
    });
    for (const known of ["direct", "exit", "unavailable", "removed"]) {
      it(`route_state ${known} is kept`, () => {
        const raw = rawOverview();
        raw.nets[0].route_state = known;
        assert.equal(parseOverview(raw).nets[0].route_state, known);
      });
    }
  });

  describe("fields of a brief of the wrong type", () => {
    it("label and id_hex that are not strings become null", () => {
      const raw = rawOverview();
      Object.assign(raw.nets[0], { label: 5, id_hex: {} });
      const n = parseOverview(raw).nets[0];
      assert.deepEqual([n.label, n.id_hex], [null, null]);
    });
    it("booleans must be exactly true", () => {
      const raw = rawOverview();
      Object.assign(raw.nets[0], { joining: "yes", proxy: 1, me_excluded: true });
      const n = parseOverview(raw).nets[0];
      assert.deepEqual([n.joining, n.proxy, n.me_excluded], [false, false, true]);
    });
    it("negative or non-finite counts become 0", () => {
      const raw = rawOverview();
      Object.assign(raw.nets[0], { peers_online: -3, peers_total: "7", members_voting: null });
      const n = parseOverview(raw).nets[0];
      assert.deepEqual([n.peers_online, n.peers_total, n.members_voting], [0, 0, 0]);
    });
    it("exits keeps only strings", () => {
      const raw = rawOverview();
      raw.nets[0].exits = ["a", 4, null, "b"];
      assert.deepEqual(parseOverview(raw).nets[0].exits, ["a", "b"]);
    });
  });

  describe("a slot or a brief with a bad id", () => {
    it("id ../x drops the brief and the slot, the others stay", () => {
      const raw = rawOverview();
      raw.nets.push({ ...brief("x"), id: "../x" });
      raw.slots.push({ slot: "../x", state: null, members: null });
      const ov = parseOverview(raw);
      assert.deepEqual(ov.nets.map((n) => n.id), [MAIN, B]);
      assert.deepEqual(ov.slots.map((s) => s.slot), [MAIN, B]);
    });
  });

  describe("a slot with a bad state or member list", () => {
    it("state that is not an object becomes null", () => {
      const raw = rawOverview();
      raw.slots[1].state = "oops";
      assert.equal(parseOverview(raw).slots[1].state, null);
    });
    it("members that are not a list become null", () => {
      const raw = rawOverview();
      raw.slots[1].members = { members: "x" };
      assert.equal(parseOverview(raw).slots[1].members, null);
    });
  });

  describe("restart_slots", () => {
    it("keeps only strings", () => {
      assert.deepEqual(parseOverview(rawOverview({ restart_slots: [MAIN, 3, B] })).restart_slots, [MAIN, B]);
    });
    it("missing reads as an empty list", () => {
      const raw = rawOverview();
      delete raw.restart_slots;
      assert.deepEqual(parseOverview(raw).restart_slots, []);
    });
  });

  describe("not an overview", () => {
    for (const [what, raw] of [["null", null], ["a number", 5], ["a string", "x"], ["an array", []], ["undefined", undefined]]) {
      it(`${what} gives null`, () => {
        assert.equal(parseOverview(raw), null);
      });
    }
    it("slots that is not an array gives null", () => {
      assert.equal(parseOverview(rawOverview({ slots: {} })), null);
    });
    it("no slots gives null", () => {
      const raw = rawOverview();
      delete raw.slots;
      assert.equal(parseOverview(raw), null);
    });
  });
});

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
    it("store.state is B's state with socks and web of main", () => {
      const { store, ov } = setup(keylessMainAndB());
      const [mainState, bState] = [ov.slots[0].state, ov.slots[1].state];
      assert.deepEqual(plain(store.state.socks), mainState.socks);
      assert.deepEqual(plain(store.state.web), mainState.web);
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

describe("slots to read the config of", () => {
  describe("a network that has just appeared", () => {
    it("is B only", () => {
      assert.deepEqual(setup(twoSpecs()).ids, [B]);
    });
  });

  describe("main", () => {
    it("is never in the list, not even on the first overview", () => {
      assert.deepEqual(setup([{ id: MAIN }]).ids, []);
    });
  });

  describe("the config of B was read and nothing changed", () => {
    it("the list is empty", () => {
      const w = setup(twoSpecs());
      markConfigRead(w.store, B);
      assert.deepEqual(acceptOverview(w.store, overviewOf(twoSpecs())), []);
    });
  });

  describe("the previous read of the config failed (nothing was recorded)", () => {
    it("B is asked for again", () => {
      const w = setup(twoSpecs());
      assert.deepEqual(acceptOverview(w.store, overviewOf(twoSpecs())), [B]);
    });
  });

  describe("config_rev of B grew", () => {
    it("B is in the list", () => {
      const w = setup(twoSpecs());
      markConfigRead(w.store, B);
      const next = acceptOverview(w.store, overviewOf([{ id: MAIN }, { id: B, state: { config_rev: 2 } }]));
      assert.deepEqual(next, [B]);
    });
  });

  describe("boot_id of B changed", () => {
    it("B is in the list", () => {
      const w = setup(twoSpecs());
      markConfigRead(w.store, B);
      const next = acceptOverview(w.store, overviewOf([{ id: MAIN }, { id: B, state: { boot_id: "boot-new" } }]));
      assert.deepEqual(next, [B]);
    });
  });

  describe("config_rev or boot_id of main changed", () => {
    it("main is still not in the list", () => {
      const w = setup(twoSpecs());
      markConfigRead(w.store, B);
      const next = acceptOverview(w.store, overviewOf([{ id: MAIN, state: { config_rev: 9, boot_id: "boot-x" } }, { id: B }]));
      assert.deepEqual(next, []);
    });
  });

  describe("a hidden network", () => {
    it("without a key and not joining is not in the list", () => {
      assert.deepEqual(setup([{ id: MAIN }, { id: B, brief: KEYLESS }]).ids, []);
    });
    it("with a null state is not in the list", () => {
      assert.deepEqual(setup([{ id: MAIN }, { id: B, state: null }]).ids, []);
    });
  });

  describe("two new networks", () => {
    it("both are in the list, in the order of the overview", () => {
      assert.deepEqual(setup([{ id: MAIN }, { id: B }, { id: C }]).ids, [B, C]);
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

describe("eachSlot", () => {
  describe("main among the slots", () => {
    it("is written first, the others in the given order", async () => {
      const w = setupWrites();
      await eachSlot(w.store, [B, C, MAIN], w.write);
      assert.deepEqual(w.written, [MAIN, B, C]);
    });
  });

  describe("the second write fails", () => {
    it("the first is written and the failure carries the label of B", async () => {
      const w = setupWrites({ fail: { [B]: failed("boom") } });
      const failures = await eachSlot(w.store, [MAIN, B], w.write);
      assert.deepEqual(w.written, [MAIN, B]);
      assert.equal(failures.length, 1);
      assert.equal(failures[0].slot, B);
      assert.equal(failures[0].label, "my devices");
      assert.equal(failures[0].error.message, "boom");
    });
    it("the text of the failure names the network", async () => {
      const w = setupWrites({ fail: { [B]: failed("boom") } });
      const [f] = await eachSlot(w.store, [MAIN, B], w.write);
      assert.equal(slotFailureText(f), '"my devices": boom');
    });
    it("a network after the failed one is still written", async () => {
      const w = setupWrites({ fail: { [B]: failed("boom") } });
      await eachSlot(w.store, [MAIN, B, C], w.write);
      assert.deepEqual(w.written, [MAIN, B, C]);
    });
  });

  describe("the write of main fails", () => {
    it("B is still written, the failure is of main", async () => {
      const w = setupWrites({ fail: { [MAIN]: failed("nope") } });
      const failures = await eachSlot(w.store, [MAIN, B], w.write);
      assert.deepEqual(w.written, [MAIN, B]);
      assert.deepEqual(failures.map((f) => f.slot), [MAIN]);
    });
  });

  describe("write throws", () => {
    it("counts as a failure with the thrown message, the rest goes on", async () => {
      const w = setupWrites({ throws: { [B]: new Error("kaboom") } });
      const failures = await eachSlot(w.store, [MAIN, B, C], w.write);
      assert.deepEqual(w.written, [MAIN, B, C]);
      assert.deepEqual(failures.map((f) => [f.slot, f.error.message]), [[B, "kaboom"]]);
    });
  });

  describe("every write succeeds", () => {
    it("the list of failures is empty", async () => {
      const w = setupWrites();
      assert.deepEqual(await eachSlot(w.store, [MAIN, B], w.write), []);
    });
  });

  describe("a network without a label", () => {
    it("is named by its key in the failure", async () => {
      const w = setupWrites({ fail: { [B]: failed("boom") }, bBrief: { label: null, id_hex: "3f2a77" } });
      const [f] = await eachSlot(w.store, [MAIN, B], w.write);
      assert.equal(f.label, "Network 3f2a");
    });
  });

  describe("the writes", () => {
    it("run one after another: the next starts after the previous one finished", async () => {
      const gate = deferred();
      const w = setupWrites({ gate: { [MAIN]: gate.promise } });
      const done = eachSlot(w.store, [B, MAIN], w.write);
      await tick();
      assert.deepEqual(w.written, [MAIN], "B was started before main finished");
      gate.resolve();
      await done;
      assert.deepEqual(w.written, [MAIN, B]);
    });
  });
});

describe("slotFailureText", () => {
  it('is "<label>": <message>', () => {
    assert.equal(slotFailureText({ slot: MAIN, label: "friends", error: { message: "boom" } }), '"friends": boom');
  });
});

// ---------------------------------------------------------------- helpers

const plain = (v) => JSON.parse(JSON.stringify(v));
const tick = () => new Promise((r) => setTimeout(r, 0));

function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

// A brief of a network with a key and the same look as in the node: B is "my devices", main is "friends".
function brief(id, over = {}) {
  return {
    id, label: id === MAIN ? "friends" : "my devices", id_hex: id === MAIN ? "3f2a01" : "9c0100", status: "online", exit_enabled: false,
    proxy: id === MAIN, route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 1, peers_total: 2, ...over,
  };
}

// The state of a slot: unique `node`, `socks` and `web` per slot, so that a mix-up of slots shows.
function stateOf(id, over = {}) {
  const base = baseState();
  const port = id === MAIN ? 1080 : 2080;
  return {
    ...base,
    boot_id: "boot-" + id,
    config_rev: 1,
    node: { ...base.node, id: "node-" + id, status: "online" },
    socks: { addr: "127.0.0.1:" + port },
    web: { access: "token", filter: id },
    route: { state: "direct", exits: [] },
    ...over,
  };
}

// An `Overview` as `parseOverview` returns it; a spec is `{ id, brief?, state?: fields | null, members? }`.
function overviewOf(specs, { restart = [] } = {}) {
  return {
    api: 2,
    nets: specs.map((s) => brief(s.id, s.brief)),
    slots: specs.map((s) => ({
      slot: s.id,
      state: s.state === null ? null : stateOf(s.id, s.state),
      members: "members" in s ? s.members : membersReply(),
    })),
    restart_slots: restart,
  };
}

// The JSON of `GET /api/overview` for main and B; `over` overrides top-level fields.
function rawOverview(over = {}) {
  return plain({ ...overviewOf([{ id: MAIN }, { id: B }]), ...over });
}

const twoSpecs = () => [{ id: MAIN }, { id: B }];
const keylessMainAndB = () => [{ id: MAIN, brief: KEYLESS }, { id: B }];

function setup(specs, opts) {
  const store = createStore();
  const ov = overviewOf(specs, opts);
  const ids = acceptOverview(store, ov);
  return { store, ov, ids };
}

// What a successful read of the config of a slot leaves in its slice.
function markConfigRead(store, id) {
  const slice = store.slots[id];
  slice.bootId = slice.state.boot_id;
  slice.configRev = slice.state.config_rev;
  slice.config = {};
}

const failed = (message) => ({ ok: false, error: { message } });

// A store with main "friends" and B, plus a `write` that records the slots it was called for.
// `fail` — slot → a `{ok:false}` result, `throws` — slot → an error to throw, `gate` — slot → a promise to wait for.
function setupWrites({ fail = {}, throws = {}, gate = {}, bBrief = {} } = {}) {
  const store = createStore();
  const specs = [{ id: MAIN }, { id: B, brief: bBrief }, { id: C, brief: { label: "work" } }];
  acceptOverview(store, overviewOf(specs));
  const written = [];
  const write = async (slot) => {
    written.push(slot);
    if (gate[slot]) await gate[slot];
    if (throws[slot]) throw throws[slot];
    return fail[slot] ?? { ok: true };
  };
  return { store, written, write };
}
