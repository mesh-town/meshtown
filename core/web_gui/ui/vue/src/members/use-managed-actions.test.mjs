/* `use-managed-actions.ts`: roles and deleting a managed network, against a fake `api`. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { useManagedActions } from "./use-managed-actions.ts";
import { PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";

const reply = () => membersReply({ rows: [selfRow({ admin: true }), peerOneRow({ admin: true })], kind: "Managed", admins: [SELF_FULL, PEER_FULL], me_admin: true });

function setup(api) {
  const store = { lead: "main", members: null, slots: {}, nets: [] };
  const status = { value: { text: "", cls: "" } };
  const wake = vi.fn();
  const env = { store, call: api, status, wake, nameOf: () => "boris", net: () => "work" };
  return { actions: useManagedActions(env), store, status, wake };
}

describe("setAdmin", () => {
  it("posts the role, keeps the new list and says who is an admin now", async () => {
    const api = vi.fn(async () => reply());
    const { actions, store, status, wake } = setup(api);
    await actions.setAdmin(PEER_FULL, true, "main");
    assert.deepEqual(api.mock.calls[0], ["POST", "/api/members/admin", { id: PEER_FULL, admin: true }]);
    assert.equal(store.members.kind, "Managed");
    assert.equal(status.value.text, 'boris is now an admin of "work"');
    assert.equal(wake.mock.calls.length, 1);
  });

  it("a refusal becomes the sentence of its code", async () => {
    const api = vi.fn(async () => { throw new ApiError(409, "not_admin"); });
    const { actions, status } = setup(api);
    await actions.setAdmin(PEER_FULL, false, "main");
    assert.equal(status.value.text, "Only admins can do this.");
    assert.equal(status.value.cls, "field-error-text");
  });
});

describe("deleteNet", () => {
  it("posts the confirmation and wakes the poller", async () => {
    const api = vi.fn(async () => ({}));
    const { actions, wake } = setup(api);
    await actions.deleteNet("main");
    assert.deepEqual(api.mock.calls[0], ["POST", "/api/network/delete", { confirm: true }]);
    assert.equal(wake.mock.calls.length, 1);
  });

  it("a refusal is shown under the list and does not wake", async () => {
    const api = vi.fn(async () => { throw new ApiError(409, "not_admin"); });
    const { actions, status, wake } = setup(api);
    await actions.deleteNet("main");
    assert.equal(status.value.text, "Only admins can do this.");
    assert.equal(wake.mock.calls.length, 0);
  });
});
