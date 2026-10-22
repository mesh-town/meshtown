/* The promise after joining when the node's new start died (`join-actions.ts`: `answeredAfterJoin`, `settlePromise`).
 * On a failed restart the node manages to change `boot_id` and show `stopping`, then goes silent: polling
 * puts `store.absence`, the snapshot stays. This is not an answer but the `STALLED_SILENT` failure.
 * A live node that is alone in the network (`offline`) or starting (`starting`) still removes the promise. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const STALLED = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is on the \"Log\" tab.";
const STALLED_SILENT = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started Mesh Town from.";
const ME = { self: true, online: true, id: "aaaa1111", name: "finch-unwary" };

describe("new start died after joining", () => {
  it("boot-2 + stopping, then absence \"silent\" — failure with the log file", async () => {
    const w = await joinedAt("boot-1");
    nodeSays(w, "boot-2", "stopping");
    w.store.absence = "silent";
    w.timers[0].fn();
    assert.equal(w.form.status, STALLED_SILENT);
  });

  it("boot-2 + stopping, then absence \"stopped\" — failure with the log file", async () => {
    const w = await joinedAt("boot-1");
    nodeSays(w, "boot-2", "stopping");
    w.store.absence = "stopped";
    w.timers[0].fn();
    assert.equal(w.form.status, STALLED_SILENT);
  });

  it("boot-2 + stopping, absence is empty — failure about the \"Log\" tab", async () => {
    const w = await joinedAt("boot-1");
    nodeSays(w, "boot-2", "stopping");
    w.timers[0].fn();
    assert.equal(w.form.status, STALLED);
  });
});

describe("new start is alive", () => {
  it("a lone live node: boot-2, offline, absence is empty — promise removed", async () => {
    const w = await joinedAt("boot-1");
    nodeSays(w, "boot-2", "offline");
    w.timers[0].fn();
    assert.equal(w.form.status, "");
  });

  it("starting with a new boot_id, absence is empty — promise removed", async () => {
    const w = await joinedAt("boot-1");
    nodeSays(w, "boot-2", "starting");
    w.timers[0].fn();
    assert.equal(w.form.status, "");
  });
});

async function joinedAt(boot) {
  const w = setup({ reply: { label: "family", restart: true }, boot });
  w.form.inviteInput = "k";
  await joinNetwork(w.form, w.store, w.deps);
  return w;
}

// A snapshot the poller put into the store.
function nodeSays(w, boot, status) {
  w.store.state = { boot_id: boot, node: { status }, peers: [ME] };
}
