/* A deadline failure is removed by a late snapshot (`join-actions.ts`: `liftOnAnswer`). The deadline expired, there was no snapshot with a new
 * `boot_id` — the page showed a failure; then the connection returned and a snapshot of a running node arrived
 * (new `boot_id`, not `stopping`, `absence` is empty) — the failure goes away, as with an answer before the deadline: the status is empty.
 * A failure not caused by the deadline (a join request error) and a new click on "Join" leave the snapshot alone. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { ApiError } from "../core/api.ts";
import { joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const STALLED = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is on the \"Log\" tab.";
const STALLED_SILENT = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started Mesh Town from.";

describe("deadline failure and a late snapshot", () => {
  it("deadline → failure → a snapshot with a new boot_id, not stopping → status is empty", async () => {
    const w = await joinedAt("boot-1");
    w.store.absence = "silent";
    w.timers[0].fn();
    assert.equal(w.form.status, STALLED_SILENT);
    w.store.absence = null;
    w.store.state = { boot_id: "boot-2", node: { status: "online" }, peers: [] };
    await nextTick();
    assert.equal(w.form.status, "");
  });

  it("a snapshot with the old boot_id does not remove the failure", async () => {
    const w = await joinedAt("boot-1");
    w.timers[0].fn();
    w.store.state = { boot_id: "boot-1", node: { status: "online" }, peers: [] };
    await nextTick();
    assert.equal(w.form.status, STALLED);
  });

  it("a snapshot with a new boot_id in stopping does not remove the failure", async () => {
    const w = await joinedAt("boot-1");
    w.timers[0].fn();
    w.store.state = { boot_id: "boot-2", node: { status: "stopping" }, peers: [] };
    await nextTick();
    assert.equal(w.form.status, STALLED);
  });

  it("a failure not from the deadline (join error) is not touched by a snapshot", async () => {
    const w = setup({ fail: new ApiError(500, "http_500", "failure") });
    w.form.inviteInput = "k";
    await joinNetwork(w.form, w.store, w.deps);
    const inviteError = w.form.inviteError;
    w.store.state = { boot_id: "boot-2", node: { status: "online" }, peers: [] };
    await nextTick();
    assert.equal(w.form.inviteError, inviteError);
    assert.equal(w.form.status, "");
  });

  it("a new click on \"Join\" cancels the wait: the old failure does not overwrite the new status", async () => {
    const w = await joinedAt("boot-1");
    w.timers[0].fn();
    await joinNetwork(w.form, w.store, w.deps);
    const promised = w.form.status;
    w.store.state = { boot_id: "boot-2", node: { status: "online" }, peers: [] };
    await nextTick();
    assert.equal(w.form.status, promised);
  });
});

async function joinedAt(boot) {
  const w = setup({ reply: { label: "family", restart: true }, boot });
  w.form.inviteInput = "k";
  await joinNetwork(w.form, w.store, w.deps);
  return w;
}
