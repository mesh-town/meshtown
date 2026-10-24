/* `members-api.ts` for managed networks. Peer calls: `members-api.test.mjs`, `members-api.post.test.mjs`.
 *
 * Contract fixed by these tests:
 *  - `postAdmin(api, idFull, admin, slot = "main")`: `POST slotPath(slot, "members/admin")` with `{ id: idFull, admin }`;
 *      resolves `{ reply, changed }` like `postMemberAction` (the reply has no `changed` key); a rejection propagates; an unreadable
 *      reply rejects with an Error.
 *  - `deleteNetwork(api, slot)`: one `POST slotPath(slot, "network/delete")` with `{ confirm: true }`; a rejection propagates.
 *  - `parseMembers` keeps `kind` ("Peer" | "Managed"), `admins` (full ids), `me_admin` and the row's `admin`; a peer reply
 *      gets none of these keys (the parsed reply stays structurally equal to the raw one).
 *  - `memberErrorText(err)` for an error with a code and no message of the node: `not_admin`, `last_admin`, `not_managed` and
 *      `peer_only` are whole sentences without the "Couldn't update the member list:" prefix; `not_member` keeps its old reason.
 *      A message of the node still wins and gets the old prefix. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { deleteNetwork, memberErrorText, parseMembers, postAdmin } from "./members-api.ts";
import { NET_A, NET_B } from "../testing/overview-fixtures.mjs";
import { PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));
const managedReply = (over = {}) =>
  membersReply({ rows: [selfRow({ admin: true }), peerOneRow({ admin: false })], kind: "Managed", admins: [SELF_FULL], me_admin: true, ...over });

describe("postAdmin", () => {
  describe("the main network", () => {
    it("posts { id, admin: true } to /api/members/admin", async () => {
      const { api, call } = setup({ reply: managedReply() });
      await postAdmin(api, PEER_FULL, true);
      assert.equal(api.mock.calls.length, 1);
      assert.deepEqual(call(0), { method: "POST", path: "/api/members/admin", body: { id: PEER_FULL, admin: true } });
    });

    it("posts admin: false to take the role away", async () => {
      const { api, call } = setup({ reply: managedReply() });
      await postAdmin(api, PEER_FULL, false, NET_A);
      assert.deepEqual(call(0), { method: "POST", path: "/api/members/admin", body: { id: PEER_FULL, admin: false } });
    });
  });

  describe("another network", () => {
    it("posts to /api/net/<slot>/members/admin", async () => {
      const { api, call } = setup({ reply: managedReply() });
      await postAdmin(api, PEER_FULL, true, NET_B);
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/" + NET_B + "/members/admin", body: { id: PEER_FULL, admin: true } });
    });
  });

  describe("the node accepted", () => {
    it("resolves the new list and whether anything changed", async () => {
      const raw = { ...managedReply(), changed: true };
      const { api } = setup({ reply: raw });
      const result = await postAdmin(api, PEER_FULL, true);
      assert.equal(result.changed, true);
      assert.ok(!("changed" in result.reply), "the reply carries `changed`");
      assert.equal(result.reply.kind, "Managed");
      assert.deepEqual(result.reply.admins, [SELF_FULL]);
    });

    it("`changed` is false when the node says nothing", async () => {
      const { api } = setup({ reply: managedReply() });
      assert.equal((await postAdmin(api, PEER_FULL, true)).changed, false);
    });
  });

  describe("the node refused or answered nonsense", () => {
    it("an api error is not swallowed", async () => {
      const { api } = setup({ fail: new ApiError(409, "not_admin") });
      await assert.rejects(postAdmin(api, PEER_FULL, true), (err) => err.status === 409 && err.code === "not_admin");
    });

    it("an unreadable reply rejects with an Error", async () => {
      const { api } = setup({ reply: { hello: "world" } });
      await assert.rejects(postAdmin(api, PEER_FULL, true), (err) => err instanceof Error);
    });
  });
});

describe("deleteNetwork", () => {
  describe("another network", () => {
    it("posts { confirm: true } to /api/net/<slot>/network/delete", async () => {
      const { api, call } = setup({ reply: {} });
      await deleteNetwork(api, NET_B);
      assert.equal(api.mock.calls.length, 1);
      assert.deepEqual(call(0), { method: "POST", path: "/api/net/" + NET_B + "/network/delete", body: { confirm: true } });
    });
  });

  describe("the main network", () => {
    it("posts to /api/network/delete", async () => {
      const { api, call } = setup({ reply: {} });
      await deleteNetwork(api, NET_A);
      assert.deepEqual(call(0), { method: "POST", path: "/api/network/delete", body: { confirm: true } });
    });
  });

  describe("the node refused", () => {
    it("an api error is not swallowed", async () => {
      const { api } = setup({ fail: new ApiError(409, "not_admin") });
      await assert.rejects(deleteNetwork(api, NET_B), (err) => err.code === "not_admin");
    });
  });
});

describe("parseMembers of a managed network", () => {
  describe("a full managed reply", () => {
    it("keeps the kind, the admins and my role", () => {
      const parsed = parseMembers(managedReply());
      assert.equal(parsed.kind, "Managed");
      assert.deepEqual(parsed.admins, [SELF_FULL]);
      assert.equal(parsed.me_admin, true);
    });

    it("keeps the role of every row", () => {
      const rows = parseMembers(managedReply()).members;
      assert.equal(rows.find((r) => r.id_full === SELF_FULL).admin, true);
      assert.equal(rows.find((r) => r.id_full === PEER_FULL).admin, false);
    });

    it("parses to a structurally equal object", () => {
      assert.deepEqual(plain(parseMembers(managedReply())), managedReply());
    });
  });

  describe("odd values", () => {
    it("`admins` keeps the strings only", () => {
      const parsed = parseMembers(managedReply({ admins: [SELF_FULL, 7, null, PEER_FULL] }));
      assert.deepEqual(parsed.admins, [SELF_FULL, PEER_FULL]);
    });

    it("a `me_admin` that is not a boolean is dropped", () => {
      assert.ok(!("me_admin" in parseMembers(managedReply({ me_admin: "yes" }))));
    });

    it("an unknown `kind` is dropped, the list stays readable", () => {
      const parsed = parseMembers(managedReply({ kind: "Hybrid" }));
      assert.ok(parsed, "the list was not parsed");
      assert.ok(parsed.kind === undefined || parsed.kind === "Peer", "an unknown kind survived: " + parsed.kind);
    });

    it("a row `admin` that is not a boolean is dropped", () => {
      const raw = managedReply({ rows: [selfRow({ admin: "yes" })] });
      assert.ok(!("admin" in parseMembers(raw).members[0]));
    });
  });

  describe("a peer reply", () => {
    it("gets none of the managed keys", () => {
      const parsed = parseMembers(membersReply());
      for (const key of ["kind", "admins", "me_admin"]) assert.ok(!(key in parsed), key + " was added");
      assert.ok(parsed.members.every((r) => !("admin" in r)), "a row got `admin`");
    });

    it("parses to a structurally equal object", () => {
      assert.deepEqual(plain(parseMembers(membersReply())), membersReply());
    });
  });

  describe("a peer reply that says so", () => {
    it("keeps kind Peer", () => {
      assert.equal(parseMembers(membersReply({ kind: "Peer" })).kind, "Peer");
    });
  });
});

describe("memberErrorText of a managed network", () => {
  const NEW_CODES = ["not_admin", "last_admin", "not_managed", "peer_only"];

  describe("only a code", () => {
    it("last_admin: tells what to do instead", () => {
      assert.equal(
        memberErrorText(new ApiError(409, "last_admin")),
        "You're the only admin: make another member an admin or delete the network.",
      );
    });

    it("not_admin: says it is for admins", () => {
      assert.equal(memberErrorText(new ApiError(409, "not_admin")), "Only admins can do this.");
    });

    it("not_managed: says the network has no admins", () => {
      assert.equal(memberErrorText(new ApiError(409, "not_managed")), "This is a peer network: it has no admins.");
    });

    it("peer_only: says it works in a peer network only", () => {
      assert.equal(memberErrorText(new ApiError(409, "peer_only")), "This only works in a peer network.");
    });

    it("not_member keeps the old reason", () => {
      assert.equal(memberErrorText(new ApiError(409, "not_member")), "Couldn't update the member list: that computer isn't in the member list");
    });

    it("never shows a raw code, and every code has its own text", () => {
      const texts = NEW_CODES.map((code) => memberErrorText(new ApiError(409, code)));
      texts.forEach((text, i) => assert.ok(!text.includes("_"), NEW_CODES[i] + " leaked a code: " + text));
      assert.equal(new Set(texts).size, NEW_CODES.length);
    });
  });

  describe("the node sent its own message", () => {
    it("the message wins, with the usual prefix", () => {
      const err = new ApiError(409, "not_admin", "not_admin", { error: "not_admin", message: "Only an admin can invite" });
      assert.equal(memberErrorText(err), "Couldn't update the member list: only an admin can invite");
    });
  });
});

// A fake `api`: answers every call with `reply` (or rejects with `fail`) and records the calls.
function setup({ reply, fail } = {}) {
  const api = vi.fn(async () => {
    if (fail) throw fail;
    return reply;
  });
  const call = (index) => {
    const [method, path, body] = api.mock.calls[index];
    return { method, path, body };
  };
  return { api, call };
}
