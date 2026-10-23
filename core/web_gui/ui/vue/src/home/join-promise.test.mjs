/* The promise after joining a network (`join-actions.ts`: `expectNode`, `settlePromise`). Joining is accepted — the page writes
 * "Done: … is restarting", and at the 20 s deadline it either removes this promise (the node answered after joining:
 * the snapshot has a new `boot_id`, the node is not silent and not `stopping`), or replaces it with a failure: `STALLED_SILENT`,
 * if `store.absence` is not null, otherwise `STALLED`. `starting`, `offline`, `online` and a snapshot without a status with a new
 * `boot_id` — a running node (a lone live node in the network holds `offline` for the whole deadline). The deadline fires by calling `timers[i].fn()`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import * as joinActions from "./join-actions.ts";
import { joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const STALLED = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is on the \"Log\" tab.";
const STALLED_SILENT = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started Mesh Town from.";
const PROMISED = /^Done: Mesh Town is restarting/;
const STATUSES = ["starting", "stopping", "offline", "online", undefined];
const WORKING = STATUSES.filter((status) => status !== "stopping");

describe("promise after joining", () => {
  describe("joining accepted", () => {
    it("one 20 s deadline is set", async () => {
      const w = await joinedAt("boot-1");
      assert.equal(w.timers.length, 1);
      assert.equal(w.timers[0].ms, 20000);
    });

    it("before the deadline the \"Done: …\" promise is in place", async () => {
      const w = await joinedAt("boot-1");
      assert.match(w.form.status, PROMISED);
    });
  });

  describe("by the deadline the snapshot has a new boot_id", () => {
    for (const status of WORKING) {
      it(`node.status ${statusName(status)} — promise removed, no failure`, async () => {
        const w = await joinedAt("boot-1");
        nodeSays(w, { boot: "boot-2", status });
        deadline(w);
        assert.equal(w.form.status, "");
      });
    }

    it("node.status stopping — \"didn't respond\" failure: the new start is already stopping", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: "boot-2", status: "stopping" });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });

    it("live node without members: offline, only you in peers — promise removed, no failure", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: "boot-2", status: "offline", peers: [ME] });
      deadline(w);
      assert.equal(w.form.status, "");
    });
  });

  describe("by the deadline boot_id is the same as at joining", () => {
    for (const status of STATUSES) {
      it(`node.status ${statusName(status)} — "didn't respond" failure`, async () => {
        const w = await joinedAt("boot-1");
        nodeSays(w, { boot: "boot-1", status });
        deadline(w);
        assert.equal(w.form.status, STALLED);
      });
    }

    it("a stale online from the previous start does not count as the node's answer — failure", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: "boot-1", status: "online", peers: [ME, PEER] });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("by the deadline the snapshot has no boot_id", () => {
    it("no snapshot at all (state = null) — failure", async () => {
      const w = await joinedAt("boot-1");
      w.store.state = null;
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });

    it("the snapshot has no boot_id field — failure, whatever node.status says", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: undefined, status: "online" });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });

    it("boot_id is not a string — failure", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: 42, status: "online" });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("no snapshot at joining", () => {
    it("a snapshot with any boot_id arrived by the deadline — promise removed", async () => {
      const w = await joinedBlind();
      nodeSays(w, { boot: "boot-7", status: "offline" });
      deadline(w);
      assert.equal(w.form.status, "");
    });

    it("no boot_id in the snapshot at joining, a string arrived by the deadline — promise removed", async () => {
      const w = await joinedWithoutBootId();
      nodeSays(w, { boot: "boot-7", status: "starting" });
      deadline(w);
      assert.equal(w.form.status, "");
    });

    it("no snapshot at the deadline either — failure", async () => {
      const w = await joinedBlind();
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("the snapshot is read at the deadline, not at joining", () => {
    it("boot_id changed after joining, before the deadline, but the node is stopping — failure", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: "boot-2", status: "stopping" });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });

    it("boot_id changed before joining, not after — failure", async () => {
      const w = await joinedAt("boot-2");
      nodeSays(w, { boot: "boot-2", status: "online" });
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("second join before the deadline", () => {
    it("the first deadline does not touch the promise of the new join", async () => {
      const w = await joinedTwice();
      deadline(w, 0);
      assert.match(w.form.status, PROMISED);
    });

    it("the second deadline compares boot_id with the second join: it did not change — failure, even though the node is online", async () => {
      const w = await joinedTwice();
      deadline(w, 0);
      deadline(w, 1);
      assert.equal(w.form.status, STALLED);
    });

    it("second deadline: boot_id changed after the second join — promise removed", async () => {
      const w = await joinedTwice();
      nodeSays(w, { boot: "boot-3", status: "starting" });
      deadline(w, 1);
      assert.equal(w.form.status, "");
    });
  });

  describe("foreign text in form.status", () => {
    for (const boot of ["boot-2", "boot-1"]) {
      it(`the deadline does not overwrite it (boot_id ${boot === "boot-1" ? "same" : "new"})`, async () => {
        const w = await joinedAt("boot-1");
        w.form.status = "Network \"family\" created.";
        nodeSays(w, { boot, status: "offline" });
        deadline(w);
        assert.equal(w.form.status, "Network \"family\" created.");
      });
    }
  });

  describe("joining not accepted", () => {
    it("the node answered with a refusal — no deadline", async () => {
      const w = setup({ fail: new ApiError(500, "http_500", "failure") });
      await enter(w);
      assert.equal(w.timers.length, 0);
    });
  });

  describe("failure text", () => {
    it("no markup, the \"Log\" tab is named in words", async () => {
      const w = await joinedAt("boot-1");
      deadline(w);
      assert.equal(w.form.status, STALLED);
      assert.doesNotMatch(w.form.status, /[<>]|href/);
      assert.match(w.form.status, /on the "Log" tab/);
    });
  });
});

describe("node is silent by the deadline (store.absence)", () => {
  describe("boot_id is the same, absence — \"silent\"", () => {
    it("the failure names the log file and the terminal window instead of the tab — verbatim text", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "silent");
      deadline(w);
      assert.equal(w.form.status, STALLED_SILENT);
    });

    it("the text does not point to the \"Log\" tab: a silent node has no new lines in it", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "silent");
      deadline(w);
      assert.doesNotMatch(w.form.status, /tab/);
    });

    it("names the meshtown.log file", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "silent");
      deadline(w);
      assert.match(w.form.status, /meshtown\.log/);
    });

    it("no markup: no tags, no links — rendered as text", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "silent");
      deadline(w);
      assert.doesNotMatch(w.form.status, /[<>]|href/);
    });
  });

  describe("boot_id is the same, absence — \"stopped\"", () => {
    it("the same failure without the \"Log\" tab", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "stopped");
      deadline(w);
      assert.equal(w.form.status, STALLED_SILENT);
    });
  });

  describe("boot_id is the same, absence — \"token\"", () => {
    it("the page gets no snapshots anyway — the same failure without the \"Log\" tab", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, "token");
      deadline(w);
      assert.equal(w.form.status, STALLED_SILENT);
    });
  });

  describe("boot_id is the same, absence — null (the node responds)", () => {
    it("the usual failure about the \"Log\" tab: Mesh Town is alive, the tab feed is readable", async () => {
      const w = await joinedAt("boot-1");
      nodeSilent(w, null);
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("absence is read at the deadline, not at joining", () => {
    it("the node went silent after joining — failure without the \"Log\" tab", async () => {
      const w = await joinedAt("boot-1");
      w.store.absence = "silent";
      deadline(w);
      assert.equal(w.form.status, STALLED_SILENT);
    });

    it("the node was silent at joining, by the deadline it answered with the same start — the usual failure about the tab", async () => {
      const w = newJoin("boot-1");
      w.store.absence = "silent";
      await enter(w);
      nodeSilent(w, null);
      deadline(w);
      assert.equal(w.form.status, STALLED);
    });
  });

  describe("by the deadline the snapshot has a new boot_id, absence — \"silent\"", () => {
    it("failure without the \"Log\" tab: the new start answered earlier but is silent by the deadline", async () => {
      const w = await joinedAt("boot-1");
      nodeSays(w, { boot: "boot-2", status: "starting" });
      w.store.absence = "silent";
      deadline(w);
      assert.equal(w.form.status, STALLED_SILENT);
    });
  });

  describe("foreign text in form.status", () => {
    it("the deadline does not overwrite it even with absence \"silent\"", async () => {
      const w = await joinedAt("boot-1");
      w.form.status = "Network \"family\" created.";
      nodeSilent(w, "silent");
      deadline(w);
      assert.equal(w.form.status, "Network \"family\" created.");
    });
  });

  describe("constant JOIN_STALLED_SILENT", () => {
    it("exported from join-actions.ts with the verbatim text", () => {
      assert.equal(joinActions.JOIN_STALLED_SILENT, STALLED_SILENT);
    });

    it("the old JOIN_STALLED remains: the node responds, but the new start did not show up", () => {
      assert.equal(joinActions.JOIN_STALLED, STALLED);
    });
  });
});

const ME = { self: true, online: true, id: "aaaa1111", name: "finch-unwary" };
const PEER = { online: true, id: "bbbb2222", name: "peer-one", path: "direct" };

function statusName(status) {
  return status === undefined ? "— no field" : status;
}

function newJoin(boot) {
  return setup({ reply: { label: "family", restart: true }, boot });
}

async function enter(w, key = "k") {
  w.form.inviteInput = key;
  await joinNetwork(w.form, w.store, w.deps);
  return w;
}

// Joining accepted when the page had seen the `boot` start.
const joinedAt = (boot) => enter(newJoin(boot));

// Joining accepted while there was no snapshot at all.
function joinedBlind() {
  const w = newJoin();
  w.store.state = null;
  return enter(w);
}

// Joining accepted with a snapshot without `boot_id`.
function joinedWithoutBootId() {
  const w = newJoin();
  delete w.store.state.boot_id;
  return enter(w);
}

// Two joins in a row; between them the node changed `boot_id` to boot-2 and stayed `online` — like a hung start.
async function joinedTwice() {
  const w = await joinedAt("boot-1");
  nodeSays(w, { boot: "boot-2", status: "online" });
  return enter(w, "k2");
}

// A snapshot the poller put into the store: whole, as it comes from the node.
function nodeSays(w, { boot, status, peers = [ME, PEER] }) {
  w.store.state = { boot_id: boot, node: status === undefined ? {} : { status }, peers };
}

// The node went silent: the page gets no snapshots because of `absence`, the snapshot is the same (`boot_id` did not change).
function nodeSilent(w, absence) {
  nodeSays(w, { boot: "boot-1", status: "stopping" });
  w.store.absence = absence;
}

// Deadline number `i` expired.
function deadline(w, i = 0) {
  w.timers[i].fn();
}
