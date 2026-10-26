/* `parseOverview`: the soft parse of `GET /api/overview`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { parseOverview } from "./overview.ts";
import { SELF_FULL } from "../testing/section.mjs";
import { B, MAIN, brief, rawOverview } from "./overview-test-support.mjs";

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

  describe("admission_refused", () => {
    const refusedOf = (v) => {
      const raw = rawOverview();
      if (v === undefined) delete raw.nets[0].admission_refused;
      else raw.nets[0].admission_refused = v;
      return parseOverview(raw).nets[0].admission_refused;
    };
    it("true stays true; false, missing and other shapes read as false", () => {
      assert.equal(refusedOf(true), true);
      assert.equal(refusedOf(false), false);
      assert.equal(refusedOf(undefined), false);
      assert.equal(refusedOf("yes"), false);
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
      Object.assign(raw.nets[0], { peers_online: -3, peers_total: "7" });
      const n = parseOverview(raw).nets[0];
      assert.deepEqual([n.peers_online, n.peers_total], [0, 0]);
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
