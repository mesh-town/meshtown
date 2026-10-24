/* Creating and joining on the Home path with network kinds (`join-actions.ts`): the kind rides in the create request only when
 * it is "Managed", the status after creating depends on it, and an invite that a regular member of a Managed network made
 * is refused with its own text. The node is a fake `api` that records every call. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createNetwork, joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const CREATE = "/api/network/create";
const MANAGED_STATUS =
  'You created "family" and you\'re its admin. Nobody else is here yet: use Invite to "family" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";

describe("createNetwork", () => {
  describe("Peer, the default", () => {
    it("sends the name and nothing else", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = " family ";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
      assert.deepEqual(w.calls, [["POST", CREATE, { label: "family" }]]);
    });

    it("an explicit Peer is sent the same way, without a kind", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "family";
      w.form.kind = "Peer";
      await createNetwork(w.form, w.store, w.deps);
      assert.deepEqual(w.calls, [["POST", CREATE, { label: "family" }]]);
    });

    it("the status is the former short sentence", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.status, 'Network "family" created.');
    });
  });

  describe("Managed", () => {
    it("sends the trimmed name and the kind", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "  family ";
      w.form.kind = "Managed";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
      assert.deepEqual(w.calls, [["POST", CREATE, { label: "family", kind: "Managed" }]]);
    });

    it("the status says you are its admin and asks for a second admin", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "family";
      w.form.kind = "Managed";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.status, MANAGED_STATUS);
    });

    it("the status names the network the node reports, not the typed text", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "fam";
      w.form.kind = "Managed";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.status, MANAGED_STATUS);
    });

    it("an empty name is refused locally, nothing is sent", async () => {
      const w = setup({});
      w.form.label = "   ";
      w.form.kind = "Managed";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "empty");
      assert.deepEqual(w.calls, []);
      assert.equal(w.form.labelError, 'Enter a name, for example "family".');
    });

    it("a refused create reports a failure, keeps the name and leaves no status", async () => {
      const w = setup({ fail: new ApiError(422, "invalid", "name is taken") });
      w.form.label = "family";
      w.form.kind = "Managed";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "failed");
      assert.equal(w.form.label, "family");
      assert.equal(w.form.labelError, "Name is taken");
      assert.equal(w.form.status, "");
    });
  });
});

describe("joinNetwork", () => {
  it("never carries a kind: the key alone is sent, whatever the form holds", async () => {
    const w = setup({ reply: { label: "friends", restart: true } });
    w.form.inviteInput = " mesh-invite2-abc ";
    w.form.kind = "Managed";
    assert.equal(await joinNetwork(w.form, w.store, w.deps), "joined");
    assert.deepEqual(w.calls, [["POST", "/api/network/join", { key: "mesh-invite2-abc" }]]);
  });

  describe("an invite that a regular member of a Managed network made (422 invite_not_from_admin)", () => {
    it("is refused with the network's name under the field", async () => {
      const w = setup({ fail: new ApiError(422, "invite_not_from_admin", "x", { label: "work" }) });
      w.form.inviteInput = "mesh-invite2-abc";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "failed");
      assert.deepEqual(w.form.inviteError, {
        message: 'This invite to "work" wasn\'t made by an admin. Ask an admin of "work" for a new one.',
        word: null,
      });
    });

    it("without a name in the reply the text names no network", async () => {
      const w = setup({ fail: new ApiError(422, "invite_not_from_admin", "x") });
      w.form.inviteInput = "mesh-invite2-abc";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "failed");
      assert.equal(w.form.inviteError.message, "This invite wasn't made by an admin. Ask an admin of the network for a new one.");
    });

    it("sets no status and no join promise", async () => {
      const w = setup({ fail: new ApiError(422, "invite_not_from_admin", "x", { label: "work" }) });
      w.form.inviteInput = "mesh-invite2-abc";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.status, "");
      assert.deepEqual(w.timers, []);
    });
  });
});
