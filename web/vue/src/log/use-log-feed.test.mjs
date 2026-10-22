/* The log polling core (`log-poller.ts`, its wrapper — `use-log-feed.ts`): a fake `api` and timer. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createLogPoller } from "./log-poller.ts";
import { POLL_MS } from "./log-view.ts";

const line = (seq) => ({ seq, t: seq, tag: "net", text: "line " + seq });

// The fixture: the node state in `env`, `api` responses are issued by the test through `respond`.
function makeEnv(over = {}) {
  const env = { active: true, boot: "b1", timers: [], cleared: [], calls: [], feeds: [], errors: [], pending: [], ...over };
  const poller = createLogPoller({
    api: (method, path) => {
      env.calls.push(path);
      return new Promise((resolve, reject) => env.pending.push({ resolve, reject }));
    },
    setTimer: (fn, ms) => { env.timers.push({ fn, ms }); return env.timers.length; },
    clearTimer: (id) => env.cleared.push(id),
    active: () => env.active,
    boot: () => env.boot,
    max: () => 2000,
    onFeed: (feed) => env.feeds.push(feed),
    onError: (text) => env.errors.push(text),
  });
  env.poller = poller;
  return env;
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

async function answer(env, data) {
  env.pending.shift().resolve(data);
  await settle();
}

describe("createLogPoller: timer", () => {
  it("start creates one timer with the POLL_MS period and makes the first request", () => {
    const env = makeEnv();
    env.poller.start();
    env.poller.start();
    assert.equal(env.timers.length, 1);
    assert.equal(env.timers[0].ms, POLL_MS);
    assert.equal(env.calls.length, 1);
  });

  it("stop clears the timer (leaving the tab or unmounting), a repeated start creates a new one", () => {
    const env = makeEnv();
    env.poller.start();
    env.poller.stop();
    assert.deepEqual(env.cleared, [1]);
    env.poller.start();
    assert.equal(env.timers.length, 2);
  });

  it("a tick on an inactive tab asks nothing", async () => {
    const env = makeEnv({ active: false });
    await env.poller.tick();
    assert.equal(env.calls.length, 0);
  });
});

describe("createLogPoller: busy", () => {
  it("a tick with an unfinished request does not breed a second", () => {
    const env = makeEnv();
    env.poller.tick();
    env.poller.tick();
    assert.equal(env.calls.length, 1);
  });

  it("busy is reset after an api rejection: the next tick asks again", async () => {
    const env = makeEnv();
    env.poller.tick();
    env.pending.shift().reject(new Error("no connection"));
    await settle();
    env.poller.tick();
    assert.equal(env.calls.length, 2);
    assert.equal(env.feeds.length, 0);
  });
});

describe("createLogPoller: polling failure", () => {
  it("status 0 — silently (an empty line), 500 — \"Couldn't update the log: …\", success clears the line", async () => {
    const env = makeEnv();
    env.poller.tick();
    env.pending.shift().reject(Object.assign(new Error("Node is not responding"), { status: 0 }));
    await settle();
    assert.deepEqual(env.errors, [""]);
    env.poller.tick();
    env.pending.shift().reject(Object.assign(new Error("Node responded with an error (code 500)"), { status: 500 }));
    await settle();
    assert.equal(env.errors[1], "Couldn't update the log: node responded with an error (code 500)");
    env.poller.tick();
    await answer(env, { lines: [] });
    assert.equal(env.errors[2], "");
  });
});

describe("createLogPoller: boot_id", () => {
  it("reset clears the feed and the cursor", async () => {
    const env = makeEnv();
    env.poller.tick();
    await answer(env, { lines: [line(1), line(2)] });
    env.poller.reset();
    assert.deepEqual(env.poller.getFeed(), { lines: [], lastSeq: 0 });
  });

  it("a response started before the boot_id change is dropped", async () => {
    const env = makeEnv();
    env.poller.tick();
    env.boot = "b2";
    env.poller.reset();
    await answer(env, { lines: [line(41), line(42)] });
    assert.deepEqual(env.poller.getFeed(), { lines: [], lastSeq: 0 });
    env.poller.tick();
    assert.match(env.calls[1], /after=0&/);
  });
});

describe("createLogPoller: pause", () => {
  it("\"Resume\" asks from the old cursor: no losses and no duplicates", async () => {
    const env = makeEnv();
    env.poller.tick();
    await answer(env, { lines: [line(1), line(2)] });
    env.poller.setPaused(true);
    await env.poller.tick();
    assert.equal(env.calls.length, 1);
    env.poller.setPaused(false);
    assert.match(env.calls[1], /after=2&/);
    await answer(env, { lines: [line(3)] });
    const seqs = env.poller.getFeed().lines.map((e) => e.seq);
    assert.deepEqual(seqs, [1, 2, 3]);
  });
});
