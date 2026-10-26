/* `members-poll.ts`: `fetchMembers(store, api)` — the second request of the poll cycle (`GET /api/members`).
 *
 * Contract fixed by these tests:
 *  - `fetchMembers(store, api): Promise<void>` never rejects, whatever the api does (even a synchronous throw).
 *  - It writes only `store.members` (`MembersReply | null`); `store.state` and the rest of the store are not touched.
 *  - No network in the snapshot (`store.state === null` or `store.state.network === null`): `store.members = null`, no request.
 *  - Reply parsed with `parseMembers`: success -> the parsed reply. A reply that cannot be parsed -> `null` (see the note below).
 *  - `ApiError` 404 and 409 -> `store.members = null` (an old node without the route, or no network/starting).
 *  - Any other failure (network, timeout, 401, 500, not an ApiError) -> the previous `store.members` stays as it was.
 *  - The page connection state (`conn`) is not an input: this function cannot light the "not responding" strip. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { fetchMembers } from "./members-poll.ts";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));
const NETWORK = { id_hex: "net-aaaa", masked: "mesh2-••••-••••" };
const STATE_WITH_NETWORK = { boot_id: "b1", network: NETWORK, peers: [] };
const OLD_REPLY = membersReply({ rows: [selfRow()] });
const NEW_REPLY = membersReply({ rows: [selfRow(), peerOneRow({ admin: false })] });

describe("fetchMembers", () => {
  describe("the snapshot has no network", () => {
    it("network is null: members become null and the node is not asked", async () => {
      const env = setup({ state: { ...STATE_WITH_NETWORK, network: null }, previous: OLD_REPLY, reply: NEW_REPLY });
      await env.run();
      assert.equal(env.store.members, null);
      assert.equal(env.api.mock.calls.length, 0, "GET /api/members must not be sent without a network");
    });

    it("no snapshot yet: members become null and the node is not asked", async () => {
      const env = setup({ state: null, previous: OLD_REPLY, reply: NEW_REPLY });
      await env.run();
      assert.equal(env.store.members, null);
      assert.equal(env.api.mock.calls.length, 0);
    });
  });

  describe("the node answers", () => {
    it("asks GET /api/members", async () => {
      const env = setup({ reply: NEW_REPLY });
      await env.run();
      assert.equal(env.api.mock.calls.length, 1);
      assert.deepEqual(env.api.mock.calls[0].slice(0, 2), ["GET", "/api/members"]);
    });

    it("the parsed reply replaces the previous one", async () => {
      const env = setup({ previous: OLD_REPLY, reply: NEW_REPLY });
      await env.run();
      assert.deepEqual(plain(env.store.members), NEW_REPLY);
    });

    it("the first reply fills an empty store.members", async () => {
      const env = setup({ previous: null, reply: NEW_REPLY });
      await env.run();
      assert.deepEqual(plain(env.store.members), NEW_REPLY);
    });

    it("the snapshot of the node is left alone", async () => {
      const env = setup({ reply: NEW_REPLY });
      await env.run();
      assert.equal(env.store.state, env.state);
    });

    // NOTE: a 200 with a body that is not a members list is neither "no route" nor "no network"; the likely reading is "null".
    it("a reply that is not a members list: members become null", async () => {
      const env = setup({ previous: OLD_REPLY, reply: { hello: "world" } });
      await env.run();
      assert.equal(env.store.members, null);
    });
  });

  describe("the node has no such list", () => {
    it.each([
      [404, "not_found"],
      [409, "busy"],
    ])("%i %s: members become null (the page works as before)", async (status, code) => {
      const env = setup({ previous: OLD_REPLY, fail: new ApiError(status, code) });
      await env.run();
      assert.equal(env.store.members, null);
    });
  });

  describe("the node is unreachable or fails", () => {
    it.each([
      ["network", new ApiError(0, "network", "Mesh Town is not responding")],
      ["timeout", new ApiError(0, "timeout", "Mesh Town didn't respond in time")],
      ["a stale token (401)", new ApiError(401, "bad_token")],
      ["a server error (500)", new ApiError(500, "internal")],
      ["a foreign exception", new TypeError("boom")],
    ])("%s: the previous members stay", async (_name, fail) => {
      const env = setup({ previous: OLD_REPLY, fail });
      await env.run();
      assert.deepEqual(plain(env.store.members), OLD_REPLY);
    });

    it("no previous members: still null", async () => {
      const env = setup({ previous: null, fail: new ApiError(0, "network") });
      await env.run();
      assert.equal(env.store.members, null);
    });
  });

  describe("it never throws", () => {
    it("a rejected request", async () => {
      const env = setup({ fail: new ApiError(500, "internal") });
      await assert.doesNotReject(env.run());
    });

    it("an api that throws before returning a promise", async () => {
      const env = setup({ throwSync: new Error("sync boom"), previous: OLD_REPLY });
      await assert.doesNotReject(env.run());
      assert.deepEqual(plain(env.store.members), OLD_REPLY);
    });
  });
});

// ---------------------------------------------------------------- setup

// A store with a snapshot (a network by default) and `previous` members; `api` answers with `reply` or fails with `fail`.
function setup({ state = STATE_WITH_NETWORK, previous = null, reply, fail, throwSync } = {}) {
  const store = createStore();
  store.state = state;
  store.members = previous;
  const api = vi.fn((_method, _path, _body) => {
    if (throwSync) throw throwSync;
    return fail ? Promise.reject(fail) : Promise.resolve(reply);
  });
  return { store, api, state: store.state, run: () => fetchMembers(store, api) };
}
