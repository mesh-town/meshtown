/* `useShell`: shell subscriptions to the bus, visibility and hash — stub `deps`, a component without DOM
 * (`withSetup`) so that `onMounted`/`onUnmounted` work. */

import { afterEach, beforeEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { nextTick } from "vue";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { useShell } from "./use-shell.ts";
import { withSetup } from "../testing/with-setup.mjs";

const POLL_STOPPED_MS = 15000;
const CONFIG_REPLY = { config: {}, rev: 1 };
const STATE = { boot_id: "b1", config_rev: 1, node: { status: "running" }, network: null, peers: [] };

describe("the meshtown:changed bus", () => {
  it("state — an extra poll of /api/state", async () => {
    const w = await mountShell();
    const before = w.count("GET /api/state");
    w.bus("state");
    await w.fireZeroTimers();
    assert.equal(w.count("GET /api/state"), before + 1);
    w.unmount();
  });

  it("config — GET /api/config", async () => {
    const w = await mountShell();
    const before = w.count("GET /api/config");
    w.bus("config");
    await flush();
    assert.equal(w.count("GET /api/config"), before + 1);
    w.unmount();
  });

  it("stopped — the line \"<program name> is stopped\", slow polling", async () => {
    const w = await mountShell();
    w.bus("stopped");
    await nextTick();
    assert.equal(w.shell.status.value.text, NET_NAME + " is stopped");
    assert.deepEqual(w.active().map((t) => t.ms), [15000], "one timer, every 15 s");
    await w.fireAllTimers();
    assert.equal(w.count("GET /api/state"), 2, "the slow poll went to the node");
    w.unmount();
  });

  it("token — the bad_token strip and polling is paused", async () => {
    const w = await mountShell();
    w.bus("token");
    await nextTick();
    assert.equal(w.shell.strip.value.kind, "bad_token");
    await w.fireAllTimers();
    assert.equal(w.count("GET /api/state"), 1, "while waiting for a token there are no requests");
    w.unmount();
  });
});

describe("unmounting", () => {
  it("removes listeners: a second event makes no requests", async () => {
    const w = await mountShell();
    w.unmount();
    const before = w.calls.length;
    w.bus("config");
    w.bus("state");
    w.win.fire("hashchange");
    w.doc.fire("visibilitychange");
    await w.fireZeroTimers();
    await flush();
    assert.equal(w.calls.length, before, "no requests are expected after unmount");
    assert.equal(w.doc.listeners("meshtown:changed"), 0);
    assert.equal(w.doc.listeners("visibilitychange"), 0);
    assert.equal(w.win.listeners("hashchange"), 0);
  });

  it("stops the poll timer", async () => {
    const w = await mountShell();
    w.unmount();
    assert.equal(w.active().length, 0);
  });
});

describe("hash navigation", () => {
  it("start — the section from the address hash, before mounting", () => {
    const w = build({ hash: "#log" });
    assert.equal(w.store.activeSection, "log");
  });

  it("unknown hash — \"Home\"", () => {
    assert.equal(build({ hash: "#nope" }).store.activeSection, "home");
  });

  it("hashchange — a new section and scroll to top", async () => {
    const w = await mountShell();
    w.deps.hash = "#ports";
    w.win.fire("hashchange");
    assert.equal(w.store.activeSection, "ports");
    assert.equal(w.scrolled, 1);
    w.deps.hash = "#nope";
    w.win.fire("hashchange");
    assert.equal(w.store.activeSection, "home");
    w.unmount();
  });
});

describe("page visibility", () => {
  it("the page is visible again — an extra poll", async () => {
    const w = await mountShell();
    w.deps.hiddenNow = false;
    w.doc.fire("visibilitychange");
    assert.equal(w.active().some((t) => t.ms === 0), true);
    w.unmount();
  });

  describe("stopped", () => {
    // Time comes from `Date.now()`; only the clock is faked, the shell timers are the stub `deps.setTimer`.
    beforeEach(() => { vi.useFakeTimers({ toFake: ["Date", "performance"] }); });
    afterEach(() => { vi.useRealTimers(); });

    it("the tab is visible again within 15 s of the stop — no poll, the planned 15 s timer stays", async () => {
      // given
      const w = await mountShell();
      w.bus("stopped");
      vi.advanceTimersByTime(POLL_STOPPED_MS - 1);
      const before = w.count("GET /api/state");
      // when
      w.doc.fire("visibilitychange");
      // then
      assert.equal(w.active().some((t) => t.ms === 0), false, "no timer at 0 ms");
      assert.deepEqual(w.active().map((t) => t.ms), [POLL_STOPPED_MS], "the planned slow poll is kept");
      assert.equal(w.count("GET /api/state"), before, "no GET /api/state");
      w.unmount();
    });

    it("the tab is visible again 15 s after the stop — poll immediately", async () => {
      // given
      const w = await mountShell();
      w.bus("stopped");
      vi.advanceTimersByTime(POLL_STOPPED_MS);
      // when
      w.doc.fire("visibilitychange");
      // then
      assert.equal(w.active().some((t) => t.ms === 0), true, "a timer at 0 ms");
      const before = w.count("GET /api/state");
      await w.fireZeroTimers();
      assert.equal(w.count("GET /api/state"), before + 1);
      w.unmount();
    });
  });

  it("waiting for a token — polling is not woken up", async () => {
    const w = await mountShell();
    w.bus("token");
    w.doc.fire("visibilitychange");
    await w.fireAllTimers();
    assert.equal(w.count("GET /api/state"), 1);
    w.unmount();
  });
});

describe("computed values", () => {
  it("ready — only when both the snapshot and the config revision exist", async () => {
    const w = build({});
    assert.equal(w.shell.ready.value, false);
    w.store.state = STATE;
    assert.equal(w.shell.ready.value, false);
    w.store.configRev = 1;
    assert.equal(w.shell.ready.value, true);
  });

  it("applyToken — token in storage, strip cleared, poll immediately", async () => {
    const w = await mountShell();
    w.bus("token");
    w.shell.applyToken("http://h/?token=fresh");
    assert.equal(w.storage.getItem("meshtown.token"), "fresh");
    assert.equal(w.shell.conn.awaitingToken, false);
    assert.equal(w.shell.strip.value, null);
    assert.equal(w.active().some((t) => t.ms === 0), true);
    w.unmount();
  });

  it("applyToken with garbage — nothing changes", async () => {
    const w = await mountShell();
    w.bus("token");
    w.shell.applyToken("not an address");
    assert.equal(w.shell.conn.awaitingToken, true);
    w.unmount();
  });
});

describe("store.absence — why the page gets no snapshots", () => {
  it("the node responds — null", async () => {
    const w = await mountShell();
    assert.equal(w.store.absence, null);
    w.unmount();
  });

  it("one failed poll — still null", async () => {
    const w = await mountShell();
    w.answerStateWith(new ApiError(0, "network"));
    await w.fireAllTimers();
    assert.equal(w.store.absence, null);
    w.unmount();
  });

  it("two failed polls in a row — \"silent\"", async () => {
    const w = await mountShell();
    w.answerStateWith(new ApiError(0, "network"));
    await w.fireAllTimers();
    await w.fireAllTimers();
    assert.equal(w.store.absence, "silent");
    w.unmount();
  });

  it("after \"silent\" a successful poll — null again", async () => {
    const w = await mountShell();
    w.answerStateWith(new ApiError(0, "network"));
    await w.fireAllTimers();
    await w.fireAllTimers();
    w.answerStateWith(null);
    await w.fireAllTimers();
    assert.equal(w.store.absence, null);
    w.unmount();
  });

  it("a 503 response — \"stopped\"", async () => {
    const w = await mountShell();
    w.answerStateWith(new ApiError(503, "stopping"));
    await w.fireAllTimers();
    assert.equal(w.store.absence, "stopped");
    w.unmount();
  });

  it("after \"stopped\" a successful poll — null again", async () => {
    const w = await mountShell();
    w.answerStateWith(new ApiError(503, "stopping"));
    await w.fireAllTimers();
    w.answerStateWith(null);
    await w.fireAllTimers();
    assert.equal(w.store.absence, null);
    w.unmount();
  });

  it("bus event stopped — \"stopped\" at once, without waiting for a tick", async () => {
    const w = await mountShell();
    w.bus("stopped");
    assert.equal(w.store.absence, "stopped");
    w.unmount();
  });

  it("bus event token — \"token\" at once, without waiting for a tick", async () => {
    const w = await mountShell();
    w.bus("token");
    assert.equal(w.store.absence, "token");
    w.unmount();
  });

  it("a new link accepted (applyToken) — \"token\" cleared", async () => {
    const w = await mountShell();
    w.bus("token");
    w.shell.applyToken("http://h/?token=fresh");
    assert.equal(w.store.absence, null);
    w.unmount();
  });
});

const flush = () => new Promise((resolve) => setImmediate(resolve));

// The event target, taking the number of listeners into account.
function fakeTarget() {
  const map = new Map();
  return {
    addEventListener: (name, fn) => map.set(name, [...(map.get(name) || []), fn]),
    removeEventListener: (name, fn) => map.set(name, (map.get(name) || []).filter((f) => f !== fn)),
    fire: (name, ev = {}) => (map.get(name) || []).slice().forEach((fn) => fn(ev)),
    listeners: (name) => (map.get(name) || []).length,
  };
}

// Harness: stub `deps`, the host component is mounted in an empty renderer (without DOM).
function build({ hash = "" } = {}) {
  const store = createStore();
  const doc = fakeTarget();
  const win = fakeTarget();
  const data = new Map();
  const storage = { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => data.set(k, v) };
  const env = { store, doc, win, storage, calls: [], timers: [], scrolled: 0, shell: null, stateReply: null };
  env.deps = {
    api: async (method, path) => {
      env.calls.push(method + " " + path);
      if (path === "/api/state") return env.stateReply ? env.stateReply() : STATE;
      if (path === "/api/config") return CONFIG_REPLY;
      throw new ApiError(404, "not_found");
    },
    setTimer: (fn, ms) => (env.timers.push({ fn, ms, live: true }), env.timers.length),
    clearTimer: (id) => { if (id && env.timers[id - 1]) env.timers[id - 1].live = false; },
    isHidden: () => Boolean(env.deps.hiddenNow),
    replaceUrl: () => {},
    scrollToTop: () => { env.scrolled++; },
    href: "http://h/",
    hash,
    storage,
    doc,
    win,
  };
  env.answerStateWith = (fail) => { env.stateReply = fail ? () => { throw fail; } : null; };
  env.active = () => env.timers.filter((t) => t.live);
  env.count = (key) => env.calls.filter((c) => c === key).length;
  env.bus = (what) => doc.fire("meshtown:changed", { detail: { what } });
  env.fireZeroTimers = async () => {
    for (const t of env.active().filter((x) => x.ms === 0)) {
      t.live = false;
      await t.fn();
    }
  };
  env.fireAllTimers = async () => {
    for (const t of env.active()) {
      t.live = false;
      await t.fn();
    }
  };
  const mounted = withSetup(() => useShell(store, env.deps));
  env.shell = mounted.result;
  env.unmount = mounted.unmount;
  return env;
}

async function mountShell(opts = {}) {
  const env = build(opts);
  await flush();
  return env;
}
