/* `poller.ts`: polling `/api/state` and loading the config — mock `api`, timers and `isHidden`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import * as connModule from "./conn.ts";
import { createConn } from "./conn.ts";
import { createPoller } from "./poller.ts";

// Via the namespace: no export — the assertion fails, not the file parse.
const POLL_STOPPED_MS = connModule.POLL_STOPPED_MS;
const CONFIG_REPLY = { config: { socks: { port: 1080 } }, rev: 1, file: { writable: true } };
const stateReply = (over = {}) => ({ boot_id: "b1", config_rev: 1, node: { status: "running" }, ...over });

describe("first snapshot", () => {
  it("no config revision — GET /api/config before store.state is written", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => stateReply();
    env.reply["GET /api/config"] = () => {
      env.stateWhenConfigAsked = env.store.state;
      return CONFIG_REPLY;
    };
    await env.poller.pollOnce();
    assert.equal(env.stateWhenConfigAsked, null, "the snapshot was written before the config");
    assert.equal(env.store.state.boot_id, "b1");
    assert.equal(env.store.configRev, 1);
    assert.deepEqual(env.calls, ["GET /api/state", "GET /api/config"]);
  });

  it("configMeta: empty arrays if the node sent no fields; file — null", async () => {
    const env = setup();
    env.reply["GET /api/config"] = () => ({ config: {}, rev: 3 });
    await env.poller.refreshConfig();
    assert.deepEqual(plain(env.store.configMeta), { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] });
    assert.equal(env.store.configFile, null);
    assert.equal(env.store.configRev, 3);
  });

  it("the revision is already known — the config is not requested on the first snapshot", async () => {
    const env = setup({ configRev: 1 });
    env.reply["GET /api/state"] = () => stateReply();
    await env.poller.pollOnce();
    assert.deepEqual(env.calls, ["GET /api/state"]);
  });
});

describe("boot_id and config_rev", () => {
  it("a boot_id change with a known previous one — a config request", async () => {
    const env = setup({ configRev: 1 });
    env.reply["GET /api/state"] = () => stateReply({ boot_id: env.boot });
    env.reply["GET /api/config"] = () => CONFIG_REPLY;
    env.boot = "b1";
    await env.poller.pollOnce();
    assert.equal(env.calls.includes("GET /api/config"), false, "the first boot_id does not ask for the config");
    env.boot = "b2";
    await env.poller.pollOnce();
    assert.equal(env.calls.includes("GET /api/config"), true);
  });

  it("config_rev greater than store.configRev — configChanged, refreshConfig clears it", async () => {
    const env = setup({ configRev: 1 });
    env.reply["GET /api/state"] = () => stateReply({ config_rev: 2 });
    env.reply["GET /api/config"] = () => ({ ...CONFIG_REPLY, rev: 2 });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, true);
    await env.poller.refreshConfig();
    assert.equal(env.conn.configChanged, false);
    assert.equal(env.store.configRev, 2);
  });

  it("the revision matched — configChanged is not set", async () => {
    const env = setup({ configRev: 1 });
    env.reply["GET /api/state"] = () => stateReply();
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, false);
  });
});

describe("failures", () => {
  it("two network errors — a strip, the next poll after backoffMs", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => { throw new ApiError(0, "network"); };
    await env.poller.pollOnce();
    await env.poller.pollOnce();
    assert.equal(env.conn.strip.kind, "disconnected");
    assert.equal(env.lastTimer().ms, env.conn.backoffMs);
    assert.equal(env.lastTimer().ms, 5000);
  });

  it("a hidden page — the next poll after 10000", async () => {
    const env = setup({ hidden: true });
    env.reply["GET /api/state"] = () => { throw new ApiError(0, "network"); };
    await env.poller.pollOnce();
    assert.equal(env.lastTimer().ms, 10000);
  });

  it("401 — the token strip, polling is not rescheduled", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => { throw new ApiError(401, "bad_token"); };
    await env.poller.pollOnce();
    assert.equal(env.conn.awaitingToken, true);
    assert.equal(env.conn.strip.kind, "bad_token");
    assert.equal(env.active().length, 0, "the timer is not set");
  });

  it("503 — \"stopped\", polling is rare: the next one in 15 s", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => { throw new ApiError(503, "stopping"); };
    await env.poller.pollOnce();
    assert.equal(env.conn.stopped, true);
    assert.deepEqual(env.active().map((t) => t.ms), [POLL_STOPPED_MS]);
    assert.equal(POLL_STOPPED_MS, 15000);
  });

  it("403 — the node's text in the strip", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => { throw new ApiError(403, "forbidden", "x", { message: "foreign address" }); };
    await env.poller.pollOnce();
    assert.deepEqual(plain(env.conn.strip), { kind: "bad_origin", text: "The page was opened from the wrong address — open it from the Mesh Town icon near the clock" });
  });

  it("another HTTP response (422) — the node is alive, the failure counter is reset", async () => {
    const env = setup();
    env.conn.failures = 1;
    env.reply["GET /api/state"] = () => { throw new ApiError(422, "bad"); };
    await env.poller.pollOnce();
    assert.equal(env.conn.failures, 0);
  });

  it("an empty response — the node is alive, the snapshot is not written, polling continues", async () => {
    const env = setup();
    env.conn.failures = 1;
    env.reply["GET /api/state"] = () => null;
    await env.poller.pollOnce();
    assert.equal(env.conn.failures, 0, "2xx — the node is alive");
    assert.equal(env.store.state, null);
    assert.equal(env.conn.strip, null);
    assert.equal(env.active().length, 1);
  });

  it("a response after two failures — \"The node restarted…\", the strip goes out after 4000 ms", async () => {
    const env = setup();
    env.reply["GET /api/state"] = () => { throw new ApiError(0, "network"); };
    await env.poller.pollOnce();
    await env.poller.pollOnce();
    env.reply["GET /api/state"] = () => stateReply();
    env.reply["GET /api/config"] = () => CONFIG_REPLY;
    await env.poller.pollOnce();
    assert.equal(env.conn.strip.kind, "restarted");
    const notice = env.active().find((t) => t.ms === 4000);
    assert.ok(notice, "the fade timer for 4000 ms");
    notice.fn();
    assert.equal(env.conn.strip, null);
  });
});

describe("refreshConfig errors", () => {
  const CASES = [
    ["401", new ApiError(401, "bad_token"), (c) => c.awaitingToken === true && c.strip.kind === "bad_token"],
    ["503", new ApiError(503, "stopping"), (c) => c.stopped === true && c.strip.kind === "stopped"],
    ["network", new ApiError(0, "network"), (c) => c.failures === 1],
  ];
  for (const [name, err, ok] of CASES) {
    it(`${name} — the connection changes by the polling rules, the config is untouched`, async () => {
      const env = setup();
      env.reply["GET /api/config"] = () => { throw err; };
      await env.poller.refreshConfig();
      assert.equal(ok(env.conn), true);
      assert.equal(env.store.configRev, null);
    });
  }
});

describe("clearPollTimer and onBus", () => {
  it("clearPollTimer clears the timer and makes no request", () => {
    const env = setup();
    env.poller.schedule(2000);
    env.poller.clearPollTimer();
    assert.equal(env.active().length, 0);
    assert.equal(env.calls.length, 0);
  });

  it("onBus(state) — poll immediately; onBus(config) — GET /api/config", async () => {
    const env = setup();
    env.reply["GET /api/config"] = () => CONFIG_REPLY;
    env.poller.onBus("state");
    assert.equal(env.active().some((t) => t.ms === 0), true);
    await env.poller.onBus("config");
    assert.deepEqual(env.calls, ["GET /api/config"]);
  });

  it("onBus(token) — the token strip, the timer is cleared, not set again", () => {
    const env = setup();
    env.poller.schedule(2000);
    env.poller.onBus("token");
    assert.equal(env.conn.strip.kind, "bad_token");
    assert.equal(env.conn.awaitingToken, true);
    assert.equal(env.active().length, 0);
  });

  it("onBus(stopped) — \"stopped\", the previous timer is replaced by a rare one (15 s)", () => {
    const env = setup();
    env.poller.schedule(2000);
    env.poller.onBus("stopped");
    assert.equal(env.conn.stopped, true);
    assert.deepEqual(env.active().map((t) => t.ms), [POLL_STOPPED_MS]);
  });

  it("onBus(unknown) — nothing", () => {
    const env = setup();
    env.poller.onBus("render");
    env.poller.onBus(undefined);
    assert.equal(env.timers.length, 0);
    assert.equal(env.conn.strip, null);
  });

  it("stopped during a request — the 200 response is discarded: stopped stays, the next poll in 15 s", async () => {
    const env = setup({ configRev: 1 });
    env.hold("GET /api/state", () => stateReply());
    const first = env.poller.pollOnce();
    env.poller.onBus("stopped");
    env.release();
    await first;
    assert.equal(env.conn.stopped, true);
    assert.equal(env.store.state, null, "the snapshot from the stale request was not written");
    assert.deepEqual(env.active().map((t) => t.ms), [POLL_STOPPED_MS]);
  });

  it("stopped during a request — a network error from it does not change the connection", async () => {
    const env = setup({ configRev: 1 });
    env.hold("GET /api/state", () => { throw new ApiError(0, "network"); });
    const first = env.poller.pollOnce();
    env.poller.onBus("stopped");
    env.release();
    await first;
    assert.equal(env.conn.failures, 0);
    assert.equal(env.conn.strip.kind, "stopped");
  });
});

describe("schedule and stop", () => {
  it("with stopped the timer is set (polling is rare, not idle)", () => {
    const env = setup();
    env.conn.stopped = true;
    env.poller.schedule(0);
    assert.deepEqual(env.active().map((t) => t.ms), [0]);
  });

  it("with awaitingToken the timer is not set", () => {
    const env = setup();
    env.conn.awaitingToken = true;
    env.poller.schedule(0);
    assert.equal(env.timers.length, 0);
  });

  it("a new schedule clears the previous timer", () => {
    const env = setup();
    env.poller.schedule(2000);
    env.poller.schedule(0);
    assert.equal(env.active().length, 1);
    assert.equal(env.active()[0].ms, 0);
  });

  it("schedule(0) during a request — exactly one retry after the response, 2 requests (not 3)", async () => {
    const env = setup({ configRev: 1 });
    env.hold("GET /api/state", () => stateReply());
    const first = env.poller.pollOnce();
    env.poller.schedule(0);
    env.poller.schedule(0);
    env.poller.pollOnce(); // a second pollOnce does not start on top of a running one
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(env.count("GET /api/state"), 1, "the second request must not start on top of a running one");
    env.release();
    await first;
    await env.runZeroTimers();
    assert.equal(env.count("GET /api/state"), 2);
  });

  it("stop() during a request — the timer is not set after the response", async () => {
    const env = setup({ configRev: 1 });
    env.hold("GET /api/state", () => stateReply());
    const first = env.poller.pollOnce();
    env.poller.stop();
    env.release();
    await first;
    assert.equal(env.active().length, 0);
  });
});

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

// Test rig: `reply["METHOD path"]` is a response function (may throw); `hold` — a response on `release()`.
function setup({ configRev = null, hidden = false } = {}) {
  const store = createStore();
  store.configRev = configRev;
  const conn = createConn();
  const env = { store, conn, reply: {}, calls: [], timers: [], held: null };
  const deps = {
    api: (method, path) => {
      const key = method + " " + path;
      env.calls.push(key);
      if (env.held && env.held.key === key) {
        const held = env.held;
        return new Promise((resolve, reject) => {
          held.resolve = () => {
            try {
              resolve(held.fn());
            } catch (err) {
              reject(err);
            }
          };
        });
      }
      const fn = env.reply[key];
      if (!fn) return Promise.reject(new Error("unexpected request " + key));
      try {
        return Promise.resolve(fn());
      } catch (err) {
        return Promise.reject(err);
      }
    },
    setTimer: (fn, ms) => {
      env.timers.push({ fn, ms, live: true });
      return env.timers.length;
    },
    clearTimer: (id) => {
      if (id && env.timers[id - 1]) env.timers[id - 1].live = false;
    },
    isHidden: () => hidden,
  };
  env.poller = createPoller(store, conn, deps);
  env.active = () => env.timers.filter((t) => t.live);
  env.lastTimer = () => env.timers[env.timers.length - 1];
  env.count = (key) => env.calls.filter((c) => c === key).length;
  env.hold = (key, fn) => {
    env.held = { key, fn, resolve: null };
  };
  env.release = () => {
    const held = env.held;
    env.held = null;
    held.resolve();
  };
  env.runZeroTimers = async () => {
    for (const t of env.active().filter((x) => x.ms === 0)) {
      t.live = false;
      await t.fn();
    }
  };
  return env;
}
