/* The slots whose config must be (re)read after an overview, and `eachSlot` / `slotFailureText`: a write to several networks. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { acceptOverview } from "./overview.ts";
import { eachSlot, slotFailureText } from "./save-config.ts";
import { B, C, KEYLESS, MAIN, brief, deferred, failed, markConfigRead, overviewOf, setup, setupWrites, tick, twoSpecs } from "./overview-test-support.mjs";

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
