/* `api.ts`: the request wrapper — token header, body parsing, errors and the signal to the bridge.
 * The stub `fetch`/`getToken`/`busTarget` go through `deps`, no page globals needed. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { api, ApiError } from "./api.ts";
import { EV_CHANGED } from "./bridge.ts";

describe("api", () => {
  describe("200 response", () => {
    it("returns the parsed JSON and sends the X-Meshtown-Token header", async () => {
      const w = setup({ reply: () => json(200, { a: 1 }), token: "tok-1" });
      const got = await api("GET", "/api/state", null, w.deps);
      assert.deepEqual(got, { a: 1 });
      assert.equal(w.calls[0].init.headers["X-Meshtown-Token"], "tok-1");
    });

    it("GET with body null — no body and no Content-Type", async () => {
      const w = setup({ reply: () => json(200, {}) });
      await api("GET", "/api/state", null, w.deps);
      assert.equal("body" in w.calls[0].init, false, "init must not have a body");
      assert.equal(headerNames(w.calls[0]).includes("content-type"), false);
    });

    it("PUT with a body — JSON and Content-Type application/json", async () => {
      const w = setup({ reply: () => json(200, {}) });
      await api("PUT", "/api/config", { a: 1 }, w.deps);
      assert.equal(w.calls[0].init.method, "PUT");
      assert.equal(w.calls[0].init.body, '{"a":1}');
      assert.equal(w.calls[0].init.headers["Content-Type"], "application/json");
    });
  });

  describe("body is not JSON or is empty", () => {
    it("204 — null", async () => {
      const w = setup({ reply: () => new Response(null, { status: 204 }) });
      assert.equal(await api("POST", "/api/x", null, w.deps), null);
    });

    it("200 with an empty body — null", async () => {
      const w = setup({ reply: () => new Response("", { status: 200 }) });
      assert.equal(await api("GET", "/api/x", null, w.deps), null);
    });

    it("200 with broken JSON — null, not an exception", async () => {
      const w = setup({ reply: () => new Response("{not json", { status: 200 }) });
      assert.equal(await api("GET", "/api/x", null, w.deps), null);
    });
  });

  describe("401 response", () => {
    it("throws ApiError with status 401 and code bad_token", async () => {
      const w = setup({ reply: () => json(401, { error: "bad_token" }) });
      const err = await rejection(api("GET", "/api/state", null, w.deps));
      assert.ok(err instanceof ApiError, "ApiError expected");
      assert.equal(err.status, 401);
      assert.equal(err.code, "bad_token");
      assert.equal(err.message, "The page was opened from an old or incomplete link — open it again: the Mesh Town icon near the clock → \"Open Mesh Town…\". Or paste the new link:");
    });

    it("sends exactly one meshtown:changed {what:'token'} to the target", async () => {
      const w = setup({ reply: () => json(401, null) });
      await rejection(api("GET", "/api/state", null, w.deps));
      assert.deepEqual(w.events, [{ what: "token" }]);
    });

    it("token changed while the request was in flight — ApiError is thrown, meshtown:changed is not sent", async () => {
      let token = "old";
      const w = setup({ reply: () => { token = "new"; return json(401, null); } });
      w.deps.getToken = () => token;
      const err = await rejection(api("GET", "/api/state", null, w.deps));
      assert.equal(err.status, 401);
      assert.deepEqual(w.events, []);
    });
  });

  describe("other server refusals", () => {
    it("422 {error:'invalid', fields} — ApiError with body.fields", async () => {
      const fields = [{ path: "node.name", message: "too long" }];
      const w = setup({ reply: () => json(422, { error: "invalid", message: "not allowed", fields }) });
      const err = await rejection(api("PUT", "/api/config", {}, w.deps));
      assert.equal(err.status, 422);
      assert.equal(err.code, "invalid");
      assert.equal(err.message, "not allowed");
      assert.deepEqual(err.body.fields, fields);
    });

    it("refusal without message — a human phrase with the status, not statusText", async () => {
      const w = setup({ reply: () => new Response('{"error":"boom"}', { status: 500, statusText: "Broken" }) });
      const err = await rejection(api("GET", "/api/x", null, w.deps));
      assert.equal(err.code, "boom");
      assert.equal(err.message, NET_NAME + " responded with an error (code 500)");
    });

    it("503 — no events on the target (the classic poll draws the strips)", async () => {
      const w = setup({ reply: () => json(503, { error: "stopping" }) });
      const err = await rejection(api("GET", "/api/state", null, w.deps));
      assert.equal(err.status, 503);
      assert.deepEqual(w.events, []);
    });

    it("403 — no events on the target", async () => {
      const w = setup({ reply: () => json(403, { error: "forbidden_origin" }) });
      await rejection(api("GET", "/api/state", null, w.deps));
      assert.deepEqual(w.events, []);
    });
  });

  describe("network and timeout", () => {
    it("fetch refused — ApiError status 0, code network, no events", async () => {
      const w = setup({ reply: () => Promise.reject(new TypeError("Failed to fetch")) });
      const err = await rejection(api("GET", "/api/state", null, w.deps));
      assert.equal(err.status, 0);
      assert.equal(err.code, "network");
      assert.equal(err.message, NET_NAME + " is not responding");
      assert.deepEqual(w.events, []);
    });

    it("fetch does not respond — ApiError status 0, code timeout, not a hang", async () => {
      const w = setup({ reply: () => new Promise(() => {}), timeoutMs: 10 });
      const err = await rejection(withDeadline(api("GET", "/api/state", null, w.deps), 1000));
      assert.ok(err instanceof ApiError, "ApiError expected");
      assert.equal(err.status, 0);
      assert.equal(err.code, "timeout");
      assert.equal(err.message, NET_NAME + " didn't respond in time");
    });
  });
});

function json(status, body) {
  return new Response(JSON.stringify(body), { status });
}

function headerNames(call) {
  return Object.keys(call.init.headers ?? {}).map((k) => k.toLowerCase());
}

/** Stub dependencies: `reply(path, init)` returns the response; `events` is what went to `target`. */
function setup({ reply, token = "t", timeoutMs = 5000 }) {
  const target = new EventTarget();
  const events = [];
  target.addEventListener(EV_CHANGED, (ev) => events.push(ev.detail));
  const calls = [];
  const fetch = (path, init) => {
    calls.push({ path, init });
    return reply(path, init);
  };
  return { calls, events, deps: { fetch, getToken: () => token, timeoutMs, busTarget: target } };
}

async function rejection(promise) {
  try {
    await promise;
  } catch (err) {
    return err;
  }
  assert.fail("an exception was expected");
}

function withDeadline(promise, ms) {
  const deadline = new Promise((_, reject) => setTimeout(() => reject(new Error("api hung")), ms));
  return Promise.race([promise, deadline]);
}
