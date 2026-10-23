/* `poller.ts`: polling a stopped node — once per 15 s (`POLL_STOPPED_MS`), not "idle".
 * The output reads as a phrase: `when_stopped > network_failure > same_strip_and_again_15_s`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import * as connModule from "./conn.ts";
import { createPoller } from "./poller.ts";

// Via the namespace: no export — the assertion fails, not the file parse.
const POLL_STOPPED_MS = connModule.POLL_STOPPED_MS;
const STATE = { boot_id: "b1", config_rev: 1, node: { status: "running" } };

describe("when stopped", () => {
  it("the rare poll constant — 15000 ms", () => {
    assert.equal(POLL_STOPPED_MS, 15000);
  });

  it("polling once per 15 s: after onBus(stopped) a timer with POLL_STOPPED_MS", () => {
    // given / when
    const env = setup();
    env.poller.onBus("stopped");
    // then
    assert.deepEqual(env.active().map((t) => t.ms), [15000]);
    assert.equal(env.conn.strip.kind, "stopped");
  });

  it("a poll in flight at onBus(state): after stopped a new GET not at once but after 15 s", async () => {
    const env = setup();
    let release;
    const gate = new Promise((r) => (release = r));
    env.reply = () => gate;
    const firing = env.poller.start();
    env.poller.onBus("state");
    env.poller.onBus("stopped");
    release(STATE);
    await firing;
    await new Promise((r) => setImmediate(r));
    assert.deepEqual(env.active().map((t) => t.ms), [15000]);
  });

  describe("network failure", () => {
    it("the same strip, the next poll again in 15 s", async () => {
      const env = setup();
      env.poller.onBus("stopped");
      env.reply = () => { throw new ApiError(0, "network"); };
      await env.fireActive();
      assert.equal(env.conn.strip.kind, "stopped", "\"No connection\" does not replace the stop strip");
      assert.equal(env.conn.stopped, true);
      assert.deepEqual(env.active().map((t) => t.ms), [15000]);
    });
  });

  describe("503", () => {
    it("the same strip, the next poll again in 15 s", async () => {
      const env = setup();
      env.poller.onBus("stopped");
      env.reply = () => { throw new ApiError(503, "stopping"); };
      await env.fireActive();
      assert.equal(env.conn.strip.kind, "stopped");
      assert.deepEqual(env.active().map((t) => t.ms), [15000]);
    });
  });

  describe("response 200", () => {
    it("stopped cleared, strip null, the next poll in 2 s", async () => {
      const env = setup();
      env.poller.onBus("stopped");
      env.reply = () => STATE;
      await env.fireActive();
      assert.equal(env.conn.stopped, false);
      assert.equal(env.conn.strip, null);
      assert.deepEqual(env.active().map((t) => t.ms), [2000]);
    });
  });

  describe("401", () => {
    it("waiting for the token: awaitingToken, the timer is not set", async () => {
      const env = setup();
      env.poller.onBus("stopped");
      env.reply = () => { throw new ApiError(401, "bad_token"); };
      await env.fireActive();
      assert.equal(env.conn.awaitingToken, true);
      assert.equal(env.active().length, 0);
    });
  });

  describe("hidden tab", () => {
    it("still 15 s, not 10", async () => {
      const env = setup({ hidden: true });
      env.poller.onBus("stopped");
      assert.deepEqual(env.active().map((t) => t.ms), [15000]);
      env.reply = () => { throw new ApiError(0, "network"); };
      await env.fireActive();
      assert.deepEqual(env.active().map((t) => t.ms), [15000]);
    });
  });
});

describe("when stopping (the stop request is about to be sent)", () => {
  it("the scheduled poll timer is cancelled: firing the live timers sends no GET /api/state", async () => {
    // given: a poll timer is scheduled
    const env = setup();
    await env.poller.start();
    assert.equal(env.active().length, 1, "a timer is scheduled after a poll");
    const before = env.count("GET /api/state");
    // when
    env.poller.onBus("stopping");
    await env.fireActive();
    // then
    assert.equal(env.active().length, 0);
    assert.equal(env.count("GET /api/state"), before, "a poll went out after stopping");
  });

  it("it is not the stopped state: no stopped flag, no stop strip", () => {
    const env = setup();
    env.poller.onBus("stopping");
    assert.equal(env.conn.stopped, false);
    assert.equal(env.conn.strip, null);
  });

  it("the response of a poll in flight is discarded", async () => {
    // given: a poll is in flight
    const env = setup();
    let release;
    const gate = new Promise((r) => (release = r));
    env.reply = () => gate;
    const firing = env.poller.start();
    // when: stopping starts, then the old response arrives
    env.poller.onBus("stopping");
    release({ ...STATE, boot_id: "late" });
    await firing;
    await new Promise((r) => setImmediate(r));
    // then
    assert.equal(env.store.state, null, "the stale snapshot was accepted");
    assert.equal(env.conn.bootId, null);
    assert.equal(env.conn.stopped, false);
    assert.equal(env.conn.strip, null);
  });

  it("\"poll again right after the current one\" is dropped", async () => {
    // given: a poll is in flight and a repeat is requested
    const env = setup();
    let release;
    const gate = new Promise((r) => (release = r));
    env.reply = () => gate;
    const firing = env.poller.start();
    env.poller.onBus("state");
    // when
    env.poller.onBus("stopping");
    release(STATE);
    await firing;
    await new Promise((r) => setImmediate(r));
    // then: no immediate (0 ms) poll is waiting
    assert.deepEqual(env.active().filter((t) => t.ms === 0), []);
  });

  it("onBus(state) afterwards resumes polling at once", async () => {
    // given
    const env = setup();
    await env.poller.start();
    env.poller.onBus("stopping");
    const before = env.count("GET /api/state");
    // when: the stop request failed and the page asks to poll again
    env.poller.onBus("state");
    await env.fireActive();
    // then
    assert.equal(env.count("GET /api/state"), before + 1);
  });
});

// Test rig: `env.reply` is what `GET /api/state` answers; `fireActive` — the live timers fired.
function setup({ hidden = false } = {}) {
  const env = { timers: [], conn: connModule.createConn(), store: createStore(), reply: () => STATE, calls: [] };
  env.store.configRev = 1;
  const deps = {
    api: async (method, path) => {
      env.calls.push(method + " " + path);
      if (path === "/api/overview") throw new ApiError(404, "not_found"); // an old node: the legacy poll
      return env.reply();
    },
    setTimer: (fn, ms) => (env.timers.push({ fn, ms, live: true }), env.timers.length),
    clearTimer: (id) => { if (id && env.timers[id - 1]) env.timers[id - 1].live = false; },
    isHidden: () => hidden,
  };
  env.poller = createPoller(env.store, env.conn, deps);
  env.count = (key) => env.calls.filter((c) => c === key).length;
  env.active = () => env.timers.filter((t) => t.live);
  env.fireActive = async () => {
    for (const t of env.active()) {
      t.live = false;
      await t.fn();
    }
  };
  return env;
}
