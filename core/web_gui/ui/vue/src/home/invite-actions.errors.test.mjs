/* The text of a failed invite request depends on the node's reply (`invite-actions.ts: fetchInvite`). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createInviteState, requestInvite } from "./invite-actions.ts";

async function failWith(fail, response) {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "net-a" }, peers: [] };
  const inv = createInviteState();
  const deps = { api: async () => { if (fail) throw fail; return response; }, clipboard: null };
  const result = await requestInvite(inv, store, deps);
  return { result, inv };
}

const CASES = [
  ["409 busy keeps the \"isn't ready yet\" text", new ApiError(409, "busy", "busy"), /^The invite isn't ready yet/],
  ["409 not_admin names the administrator", new ApiError(409, "not_admin", "x"), /^Only an administrator of this network can create invitations\.$/],
  ["409 not_in_network says the device left", new ApiError(409, "not_in_network", "x"), /^This device is no longer in this network, so it can't create invitations\.$/],
  ["422 invalid asks for a shorter name", new ApiError(422, "invalid", "x"), /^The name for this invitation is too long or has unsupported characters\. Use a shorter plain name\.$/],
  ["no reply (network) says Mesh Town is unreachable", new ApiError(0, "network", "Failed to fetch"), /^Couldn't reach Mesh Town on this device\. Check that it is running, then try again\.$/],
  ["404 quotes the message", new ApiError(404, "not_found", "no such route"), /^Couldn't create the invitation: no such route\.$/],
  ["500 with a message quotes it", new ApiError(500, "internal", "disk is full"), /^Couldn't create the invitation: disk is full\.$/],
  ["500 without a message asks to retry", new ApiError(500, "internal", ""), /^Couldn't create the invitation\. Try again in a minute\.$/],
];

describe("requestInvite: the reply code picks the text", () => {
  for (const [name, fail, expected] of CASES) {
    it(name, async () => {
      const { result, inv } = await failWith(fail);
      assert.equal(result, "failed");
      assert.match(inv.status, expected);
      assert.equal(inv.text, "");
    });
  }

  it("success with an empty invite keeps the \"isn't ready yet\" text", async () => {
    const { result, inv } = await failWith(null, {});
    assert.equal(result, "failed");
    assert.match(inv.status, /^The invite isn't ready yet/);
  });
});
