/* `poller.ts`: when the "settings were changed elsewhere" flag (`conn.configChanged`) is raised —
 * only for a snapshot newer than the revision this tab knows. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createConn } from "./conn.ts";
import { createPoller } from "./poller.ts";

const CONFIG_REPLY = { config: {}, rev: 1, file: { writable: true } };
const stateReply = (over = {}) => ({ boot_id: "b1", config_rev: 1, node: { status: "running" }, ...over });

describe("snapshot revision against the known one", () => {
  it("lower revision (the snapshot lags behind) — the flag is not raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 4 });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, false);
  });

  it("equal revision — the flag is not raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 5 });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, false);
  });

  it("greater revision — the flag is raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 6 });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, true);
  });
});

describe("own save", () => {
  it("lagging snapshot, then the caught-up one — the flag is never raised", async () => {
    // given: the tab knows revision 1 and the snapshot agrees
    const env = setup({ configRev: 1 });
    env.state = stateReply({ config_rev: 1 });
    await env.poller.pollOnce();
    // when: the save reply moves the revision, the snapshot is rebuilt later
    env.store.configRev = 2;
    env.state = stateReply({ config_rev: 1 });
    await env.poller.pollOnce();
    // then
    assert.equal(env.conn.configChanged, false, "the lagging snapshot raised the flag");
    env.state = stateReply({ config_rev: 2 });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, false, "the caught-up snapshot raised the flag");
  });
});

describe("node restart (boot_id changes)", () => {
  it("the flag is not raised while the config is being reloaded, even if the revision differs", async () => {
    // given
    const env = setup({ configRev: 1 });
    env.state = stateReply({ boot_id: "b1", config_rev: 1 });
    await env.poller.pollOnce();
    env.hold("GET /api/config", () => ({ ...CONFIG_REPLY, rev: 7 }));
    env.state = stateReply({ boot_id: "b2", config_rev: 7 });
    // when: the config request is still in flight after the poll
    await env.poller.pollOnce();
    // then
    assert.equal(env.count("GET /api/config"), 1, "the config was reloaded");
    assert.equal(env.conn.configChanged, false, "the flag blinked during the reload");
    env.release();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(env.conn.configChanged, false);
    assert.equal(env.store.configRev, 7);
  });
});

describe("node start (status starting)", () => {
  it("greater revision while the node is starting, same boot_id — the flag is not raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 6, node: { status: "starting" } });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, false);
  });

  it("greater revision, status online, same boot_id — the flag is raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 6, node: { status: "online" } });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, true);
  });

  it("network creation: starting without a flag, then the restart reloads the config, then a real foreign edit still raises the flag", async () => {
    // given: the tab knows revision 1
    const env = setup({ configRev: 1 });
    env.state = stateReply({ boot_id: "b1", config_rev: 1, node: { status: "online" } });
    await env.poller.pollOnce();
    // when: the file is written (revision grows) and the node is starting, boot_id is still the old one
    env.state = stateReply({ boot_id: "b1", config_rev: 2, node: { status: "starting" } });
    for (let i = 0; i < 3; i++) {
      await env.poller.pollOnce();
      assert.equal(env.conn.configChanged, false, "the flag blinked while starting (poll " + i + ")");
    }
    // when: the node comes up under a new boot_id
    env.configReply = { ...CONFIG_REPLY, rev: 2 };
    env.state = stateReply({ boot_id: "b2", config_rev: 2, node: { status: "starting" } });
    await env.poller.pollOnce();
    await new Promise((resolve) => setImmediate(resolve));
    // then: the config is reloaded, no flag
    assert.equal(env.count("GET /api/config"), 1, "the config was reloaded after the boot_id change");
    assert.equal(env.store.configRev, 2);
    assert.equal(env.conn.configChanged, false);
    // when: another tab edits the settings — revision grows, same boot_id, node is up
    env.state = stateReply({ boot_id: "b2", config_rev: 3, node: { status: "online" } });
    await env.poller.pollOnce();
    // then: the real foreign edit is not swallowed
    assert.equal(env.conn.configChanged, true);
  });

  it("running status, greater revision, same boot_id — the flag is raised", async () => {
    const env = setup({ configRev: 5 });
    env.state = stateReply({ config_rev: 6, node: { status: "running" } });
    await env.poller.pollOnce();
    assert.equal(env.conn.configChanged, true);
  });
});

function setup({ configRev = null } = {}) {
  const store = createStore();
  store.configRev = configRev;
  const conn = createConn();
  const env = { store, conn, state: stateReply(), calls: [], held: null, configReply: CONFIG_REPLY };
  const deps = {
    api: (method, path) => {
      const key = method + " " + path;
      env.calls.push(key);
      if (key === "GET /api/overview") return Promise.reject(new ApiError(404, "not_found")); // an old node: the legacy poll
      if (env.held && env.held.key === key) {
        const held = env.held;
        return new Promise((resolve) => { held.resolve = () => resolve(held.fn()); });
      }
      if (key === "GET /api/state") return Promise.resolve(env.state);
      if (key === "GET /api/config") return Promise.resolve(env.configReply);
      return Promise.reject(new Error("unexpected request " + key));
    },
    setTimer: () => 1,
    clearTimer: () => {},
    isHidden: () => false,
  };
  env.poller = createPoller(store, conn, deps);
  env.count = (key) => env.calls.filter((c) => c === key).length;
  env.hold = (key, fn) => { env.held = { key, fn, resolve: null }; };
  env.release = () => {
    const held = env.held;
    env.held = null;
    held.resolve();
  };
  return env;
}
