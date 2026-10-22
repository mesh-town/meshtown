/* `poller.ts`: the token was changed ("Revoke" in this tab) while a request was in flight — a 401 does not break the connection. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createConn } from "./conn.ts";
import { createPoller } from "./poller.ts";

function setup(rotateInRequest) {
  const env = { token: "old", timers: [], conn: createConn(), store: createStore() };
  env.store.configRev = 1;
  const deps = {
    api: async () => {
      if (rotateInRequest) env.token = "new";
      throw new ApiError(401, "bad_token");
    },
    getToken: () => env.token,
    setTimer: (fn, ms) => env.timers.push({ fn, ms, live: true }),
    clearTimer: (id) => { if (id) env.timers[id - 1].live = false; },
    isHidden: () => false,
  };
  env.poller = createPoller(env.store, env.conn, deps);
  env.active = () => env.timers.filter((t) => t.live);
  return env;
}

describe("401 and token change", () => {
  it("the token changed during a request — the connection is not token, polling continues", async () => {
    const env = setup(true);
    await env.poller.pollOnce();
    assert.equal(env.conn.awaitingToken, false);
    assert.equal(env.conn.strip, null);
    assert.equal(env.active().length, 1, "the next poll is scheduled");
  });

  it("the token did not change — the token strip, no timer", async () => {
    const env = setup(false);
    await env.poller.pollOnce();
    assert.equal(env.conn.awaitingToken, true);
    assert.equal(env.conn.strip.kind, "bad_token");
    assert.equal(env.active().length, 0);
  });
});
