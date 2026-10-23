/* `poller.ts` with `GET /api/overview`: one request per cycle, the fallback to `/api/state` + `/api/members` for a node
 * without the overview, slot config reads. The node is the fake node (`testing/fake-node.mjs`) behind the real `api()`. */

import { describe, expect, it } from "vitest";
import { api as coreApi } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import { createConn } from "./conn.ts";
import { createPoller } from "./poller.ts";

const OVERVIEW = "GET /api/overview";
const STATE = "GET /api/state";
const MEMBERS = "GET /api/members";
const CONFIG = "GET /api/config";
const NET_B_CONFIG = "GET /api/net/" + NET_B + "/config";
const UNREACHABLE = { path: "/api/overview", status: 500, body: { error: "internal" } };

describe("one network", () => {
  describe("a steady poll cycle", () => {
    it("sends exactly one GET /api/overview and neither /api/state nor /api/members", async () => {
      const env = setup(oneNet());
      await env.cycle();
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW]);
    });

    it("the first cycle also loads the main config, but still not /api/state or /api/members", async () => {
      const env = setup(oneNet());
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW, CONFIG]);
    });

    it("marks the store as fed by the overview", async () => {
      const env = setup(oneNet());
      await env.cycle();
      expect(env.store.source).toBe("overview");
      expect(env.store.state.boot_id).toBe("boot-1");
    });
  });

  describe("no config revision yet", () => {
    it("GET /api/config is asked before store.state is written", async () => {
      const env = setup(oneNet());
      env.node.routes[CONFIG] = () => {
        env.stateWhenConfigAsked = env.store.state;
        return { rev: 1, config: {}, file: null };
      };
      await env.cycle();
      expect(env.stateWhenConfigAsked, "the snapshot was written before the config").toBeNull();
      expect(env.store.state).not.toBeNull();
    });
  });
});

describe("two networks", () => {
  describe("the second network is the open tab, then another section is opened", () => {
    it("every cycle costs the same single GET /api/overview", async () => {
      const env = setup(twoNets());
      await env.cycle();
      env.store.peersTab = NET_B;
      env.store.activeSection = "peers";
      const onTab = await env.cycle();
      env.store.activeSection = "log";
      const onOtherSection = await env.cycle();
      expect(onTab).toEqual([OVERVIEW]);
      expect(onOtherSection).toEqual(onTab);
    });

    it("costs the same as a node with one network", async () => {
      const single = setup(oneNet());
      await single.cycle();
      const double = setup(twoNets());
      await double.cycle();
      expect(await double.cycle()).toEqual(await single.cycle());
    });
  });

  describe("the main network has no key", () => {
    it("the state of the other network leads, with socks and web of the main one", async () => {
      const env = setup(keylessMain());
      await env.cycle();
      expect(env.store.lead).toBe(NET_B);
      expect(env.store.state.network.id_hex).toBe("net-bbbb");
      expect(env.store.state.socks).toEqual({ port: 1080 });
      expect(env.store.state.web).toEqual({ port: 7070 });
    });
  });
});

describe("a node without the overview", () => {
  describe("GET /api/overview answers 404 not_found", () => {
    it("the same cycle falls back to /api/state and /api/members, the source is legacy", async () => {
      const env = setup(legacyNode());
      const calls = await env.cycle();
      expect(calls.slice(0, 2)).toEqual([OVERVIEW, STATE]);
      expect(calls).toContain(MEMBERS);
      expect(env.store.source).toBe("legacy");
      expect(env.store.state.boot_id).toBe("boot-1");
    });

    it("the following cycles do not try the overview again", async () => {
      const env = setup(legacyNode());
      await env.cycle();
      const calls = await env.cycle();
      expect(calls).toEqual([STATE, MEMBERS]);
    });

    it("a changed boot_id makes the cycle after it ask for the overview again", async () => {
      const env = setup(legacyNode());
      await env.cycle();
      env.node.legacy = false; // the node was upgraded
      env.node.state.boot_id = "boot-2";
      const restartCycle = await env.cycle();
      expect(restartCycle).not.toContain(OVERVIEW);
      const next = await env.cycle();
      expect(next[0]).toBe(OVERVIEW);
      expect(env.store.source).toBe("overview");
      expect(env.store.state.boot_id).toBe("boot-2");
    });
  });

  describe("GET /api/overview answers 404 without an error code", () => {
    it("is not a legacy node: /api/state is not asked, the overview is retried", async () => {
      const env = setup(oneNet());
      env.node.routes[OVERVIEW] = { status: 404, body: {} };
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW]);
      expect(env.store.source).toBeNull();
      delete env.node.routes[OVERVIEW];
      expect((await env.cycle())[0]).toBe(OVERVIEW);
      expect(env.store.source).toBe("overview");
    });
  });
});

describe("a failing overview", () => {
  describe("GET /api/overview answers 500", () => {
    it("is not a legacy node: no /api/state, the overview is retried and recovers", async () => {
      const env = setup(oneNet());
      env.node.fail = UNREACHABLE;
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW]);
      expect(env.store.source, "a 500 must not switch to legacy").toBeNull();
      env.node.fail = null;
      const next = await env.cycle();
      expect(next[0]).toBe(OVERVIEW);
      expect(env.store.source).toBe("overview");
    });

    // An overview error is handled like one on /api/state: the connection must end up exactly as after a
    // 500 on /api/state, so this test compares the two instead of pinning one reading.
    it("leaves the connection exactly as a 500 on /api/state does", async () => {
      const viaOverview = setup(oneNet());
      viaOverview.conn.failures = 1;
      viaOverview.node.fail = UNREACHABLE;
      await viaOverview.cycle();
      const viaState = setup(legacyNode());
      viaState.conn.failures = 1;
      viaState.node.fail = { path: "/api/state", status: 500, body: { error: "internal" } };
      await viaState.cycle();
      expect(connView(viaOverview.conn)).toEqual(connView(viaState.conn));
    });
  });

  describe("the node does not answer at all", () => {
    it("counts a failure and does not switch to legacy", async () => {
      const env = setup(oneNet());
      env.node.fail = "down";
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW]);
      expect(env.conn.failures).toBe(1);
      env.node.fail = null;
      expect((await env.cycle())[0]).toBe(OVERVIEW);
    });
  });
});

describe("generations", () => {
  describe("the node is being stopped while the overview is in flight", () => {
    it("the late answer does not touch the store", async () => {
      const env = setup(oneNet());
      env.holdOverview();
      const cycle = env.cycle();
      env.poller.onBus("stopping");
      env.release();
      await cycle;
      expect(env.store.state).toBeNull();
      expect(env.store.source).toBeNull();
    });
  });

  describe("401 for a token that was replaced during the request", () => {
    it("does not ask for a new token", async () => {
      const env = setup(oneNet());
      env.node.fail = { path: "/api/overview", status: 401, body: { error: "bad_token" } };
      env.holdOverview();
      const cycle = env.cycle();
      env.token = "token-2";
      env.release();
      await cycle;
      expect(env.conn.awaitingToken).toBe(false);
      expect(env.conn.strip).toBeNull();
    });
  });
});

describe("the config of an added network", () => {
  describe("the network appears in the overview", () => {
    it("is read once with a single GET /api/net/<slot>/config, the slice is filled", async () => {
      const env = setup(twoNets());
      const first = await env.cycle();
      expect(env.count(first, NET_B_CONFIG)).toBe(1);
      expect(env.store.slots[NET_B].config).toEqual(baseConfig({ ports: { published: [{ port: 8080 }], opened: [] } }));
      expect(env.store.slots[NET_B].configRev).toBe(1);
      expect(env.count(first, "GET /api/net/" + NET_A + "/config"), "main has no slot address").toBe(0);
    });
  });

  describe("nothing changed", () => {
    it("is not read again", async () => {
      const env = setup(twoNets());
      await env.cycle();
      const calls = await env.cycle();
      expect(calls).toEqual([OVERVIEW]);
    });
  });

  describe("config_rev grew", () => {
    it("is read again", async () => {
      const env = setup(twoNets());
      await env.cycle();
      slotOf(env.node, NET_B).rev = 2;
      const calls = await env.cycle();
      expect(env.count(calls, NET_B_CONFIG)).toBe(1);
      expect(env.store.slots[NET_B].configRev).toBe(2);
    });
  });

  describe("the read failed", () => {
    it("is repeated in the next cycle, the connection shows no error", async () => {
      const env = setup(twoNets());
      slotOf(env.node, NET_B).routes[CONFIG] = { status: 500, body: { error: "internal" } };
      await env.cycle();
      expect(env.store.slots[NET_B].config).toBeNull();
      expect(env.conn.strip).toBeNull();
      expect(env.conn.failures).toBe(0);
      delete slotOf(env.node, NET_B).routes[CONFIG];
      const next = await env.cycle();
      expect(env.count(next, NET_B_CONFIG)).toBe(1);
      expect(env.store.slots[NET_B].config).not.toBeNull();
    });
  });
});

const slotOf = (node, id) => node.slotList.find((s) => s.id === id);
const connView = (conn) => ({ failures: conn.failures, strip: conn.strip?.kind ?? null, stopped: conn.stopped, awaitingToken: conn.awaitingToken });

function oneNet() {
  return { state: baseState(), config: baseConfig() };
}

function legacyNode() {
  return { ...oneNet(), legacy: true };
}

// The main network has no key; the second one has. The process-level `socks` and `web` blocks live in the main state.
function keylessMain() {
  const opts = twoNets();
  const main = { ...opts.slots[0], id_hex: null, label: null };
  const state = { ...opts.state, network: null, socks: { port: 1080 }, web: { port: 7070 } };
  return { ...opts, state, slots: [main, opts.slots[1]] };
}

// A poller on the fake node behind the real `api()`; `cycle()` runs one poll and returns the requests it made.
function setup(nodeOptions) {
  const node = createFakeNode(nodeOptions);
  const store = createStore();
  const conn = createConn();
  const env = { node, store, conn, token: "token-1", gate: null, timers: [] };
  const fetchOnNode = (input, init) => node.fetch(input, init);
  const api = async (method, path, body) => {
    if (env.gate && path === "/api/overview") await env.gate.promise;
    return coreApi(method, path, body, { fetch: fetchOnNode, getToken: () => env.token, timeoutMs: 1000, busTarget: undefined });
  };
  const deps = { api, getToken: () => env.token, setTimer: (fn, ms) => env.timers.push({ fn, ms }), clearTimer: () => {}, isHidden: () => false };
  env.poller = createPoller(store, conn, deps);
  env.cycle = async () => {
    const mark = node.calls.length;
    await env.poller.pollOnce();
    return node.calls.slice(mark).map((c) => c.method + " " + c.path);
  };
  env.count = (calls, key) => calls.filter((c) => c === key).length;
  env.holdOverview = () => {
    env.gate = {};
    env.gate.promise = new Promise((resolve) => { env.gate.open = resolve; });
  };
  env.release = () => env.gate.open();
  return env;
}
