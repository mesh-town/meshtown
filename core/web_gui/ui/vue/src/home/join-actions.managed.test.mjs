/* Creating and joining on the Home path (`join-actions.ts`): the create request carries the name only and the status after
 * creating tells the creator to name a second admin. The node is a fake `api` that records every call. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createNetwork, joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const CREATE = "/api/network/create";
const STATUS =
  'You created "family" and you\'re its admin. Nobody else is here yet: use Invite to "family" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";

describe("createNetwork", () => {
  it("sends the trimmed name and nothing else", async () => {
    const w = setup({ reply: { key: "k", label: "family" } });
    w.form.label = "  family ";
    assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
    assert.deepEqual(w.calls, [["POST", CREATE, { label: "family" }]]);
  });

  it("the status says you are the admin and asks for a second admin", async () => {
    const w = setup({ reply: { key: "k", label: "family" } });
    w.form.label = "family";
    await createNetwork(w.form, w.store, w.deps);
    assert.equal(w.form.status, STATUS);
  });

  it("the status names the network the node reports, not the typed text", async () => {
    const w = setup({ reply: { key: "k", label: "family" } });
    w.form.label = "fam";
    await createNetwork(w.form, w.store, w.deps);
    assert.equal(w.form.status, STATUS);
  });

  it("an empty name is refused locally, nothing is sent", async () => {
    const w = setup({});
    w.form.label = "   ";
    assert.equal(await createNetwork(w.form, w.store, w.deps), "empty");
    assert.deepEqual(w.calls, []);
    assert.equal(w.form.labelError, 'Enter a name, for example "family".');
  });

  it("a refused create reports a failure, keeps the name and leaves no status", async () => {
    const w = setup({ fail: new ApiError(422, "invalid", "name is taken") });
    w.form.label = "family";
    assert.equal(await createNetwork(w.form, w.store, w.deps), "failed");
    assert.equal(w.form.label, "family");
    assert.equal(w.form.labelError, "Name is taken");
    assert.equal(w.form.status, "");
  });
});

describe("joinNetwork", () => {
  it("the key alone is sent", async () => {
    const w = setup({ reply: { label: "friends", restart: true } });
    w.form.inviteInput = " mesh-invite2-abc ";
    assert.equal(await joinNetwork(w.form, w.store, w.deps), "joined");
    assert.deepEqual(w.calls, [["POST", "/api/network/join", { key: "mesh-invite2-abc" }]]);
  });
});
