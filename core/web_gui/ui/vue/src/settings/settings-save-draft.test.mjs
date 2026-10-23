/* `saveDraft`: what saving does to the draft if input happened while the PUT was in flight.
 * The output reads as a phrase: `saveDraft > input_during_PUT > is_not_erased`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createSettingsForm, editField, restartNode, saveDraft } from "./settings-actions.ts";
import { restartTargets } from "./settings-view.ts";

// `sent` — the key that went into the PUT; `later` — the key typed after sending.
const SENT = "checks.enabled";
const LATER = "socks.port";

describe("saveDraft", () => {
  describe("input during PUT", () => {
    it("is not erased: both the edit of the sent field and the new field remain", async () => {
      // given
      const w = setup();
      editField(w.form, SENT, false);
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, SENT, true);
      editField(w.form, LATER, "1090");
      w.reply.resolve();
      assert.equal(await saving, "saved");
      // then
      assert.deepEqual(plain(w.form.draft), { [SENT]: true, [LATER]: "1090" });
    });

    it("the \"Saved\" text depends on the sent keys, not on those typed later", async () => {
      // given: `socks.port` requires a restart, `checks.enabled` does not
      const w = setup();
      editField(w.form, SENT, false);
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, LATER, "1090");
      w.reply.resolve();
      await saving;
      // then
      assert.equal(w.form.message.text, "Saved");
    });

    it("PUT failed: the draft is intact together with the input made during the PUT", async () => {
      const w = setup();
      editField(w.form, SENT, false);
      const saving = saveDraft(w.form, w.store, w.deps);
      editField(w.form, LATER, "1090");
      w.reply.reject(new ApiError(0, "network"));
      assert.notEqual(await saving, "saved");
      assert.deepEqual(plain(w.form.draft), { [SENT]: false, [LATER]: "1090" });
    });
  });

  describe("no input", () => {
    it("draft is empty", async () => {
      // given
      const w = setup();
      editField(w.form, SENT, false);
      editField(w.form, LATER, "1090");
      // when
      const saving = saveDraft(w.form, w.store, w.deps);
      w.reply.resolve();
      await saving;
      // then
      assert.deepEqual(plain(w.form.draft), {});
    });
  });
});

describe("saveDraft over several networks", () => {
  describe("one network", () => {
    it("checks.enabled edited: exactly one PUT /api/config and nothing else", async () => {
      // given
      const w = setupNets("one");
      editField(w.form, "checks.enabled", false);
      // when
      const outcome = await saveDraft(w.form, w.store, w.deps);
      // then
      assert.equal(outcome, "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config"]);
    });

    it("no overview at all (old node): calls are the same", async () => {
      const w = setupNets("legacy");
      editField(w.form, "checks.enabled", false);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config"]);
    });
  });

  describe("a lone secondary network (the main one holds no key)", () => {
    it("shared path edited: the main PUT and the PUT of the secondary network", async () => {
      const w = setupNets("lone");
      editField(w.form, "checks.interval_sec", "900");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT ${BPATH}`]);
    });
  });

  describe("two networks", () => {
    it("shared path edited: the main PUT first, then the PUT of the other network", async () => {
      const w = setupNets("two");
      editField(w.form, "checks.interval_sec", "900");
      const outcome = await saveDraft(w.form, w.store, w.deps);
      assert.equal(outcome, "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT /api/net/${B}/config`]);
    });

    it("the main PUT carries the whole config of the main network", async () => {
      const w = setupNets("two");
      editField(w.form, "checks.interval_sec", "900");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.calls[0].body.config), { ...MAIN_CONFIG, checks: { ...MAIN_CONFIG.checks, interval_sec: 900 } });
    });

    it("the other network gets the edited shared path and keeps the rest of its own config", async () => {
      const w = setupNets("two");
      editField(w.form, "checks.interval_sec", "900");
      await saveDraft(w.form, w.store, w.deps);
      const sent = plain(w.calls[1].body.config);
      assert.equal(sent.checks.interval_sec, 900);
      assert.deepEqual({ ...sent, checks: { ...sent.checks, interval_sec: 0 } }, { ...SLOT_CONFIG, checks: { ...SLOT_CONFIG.checks, interval_sec: 0 } });
    });

    it("a process path edited together with a shared one does not reach the other network", async () => {
      const w = setupNets("two");
      editField(w.form, "checks.enabled", false);
      editField(w.form, "socks.port", "1090");
      await saveDraft(w.form, w.store, w.deps);
      const sent = plain(w.calls[1].body.config);
      assert.equal(sent.checks.enabled, false);
      assert.equal(sent.socks.port, SLOT_CONFIG.socks.port, "the process path went into the other network");
    });

    it("only a process path edited: no PUT to the other network", async () => {
      const w = setupNets("two");
      editField(w.form, "socks.port", "1090");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config"]);
    });

    it("the main PUT failed: the other network is not written and the draft stays", async () => {
      const w = setupNets("two", { "PUT /api/config": [() => { throw new ApiError(500, "http", "boom"); }] });
      editField(w.form, "checks.interval_sec", "900");
      const outcome = await saveDraft(w.form, w.store, w.deps);
      assert.equal(outcome, "failed");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config"]);
      assert.deepEqual(plain(w.form.draft), { "checks.interval_sec": "900" });
    });

    it("the other network has no key yet (joining): only the main PUT", async () => {
      const w = setupNets("two", {}, { bJoining: true });
      editField(w.form, "checks.interval_sec", "900");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config"]);
    });

    it("stale revision of the other network: one GET of its config and one repeated PUT", async () => {
      const w = setupNets("two", {
        [`PUT ${BPATH}`]: [() => { throw new ApiError(409, "stale_rev", "stale", { error: "stale_rev" }); }, null],
        [`GET ${BPATH}`]: [() => ({ config: structuredClone(SLOT_CONFIG), rev: 20 })],
      });
      editField(w.form, "checks.interval_sec", "900");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT ${BPATH}`, `GET ${BPATH}`, `PUT ${BPATH}`]);
      assert.equal(w.calls[3].body.rev, 20);
    });
  });

  describe("three networks", () => {
    it("each other network is written after the main one, in the order of the overview", async () => {
      const w = setupNets("three");
      editField(w.form, "checks.enabled", false);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT ${BPATH}`, `PUT ${CPATH}`]);
    });

    it("one of the others fails: the next one is still written", async () => {
      const w = setupNets("three", { [`PUT ${BPATH}`]: [() => { throw new ApiError(500, "http", "boom"); }] });
      editField(w.form, "checks.enabled", false);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "partial");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT ${BPATH}`, `PUT ${CPATH}`]);
    });
  });

  describe("the other network fails", () => {
    it("the outcome is partial and the main network is saved", async () => {
      const w = failingB();
      const outcome = await saveDraft(w.form, w.store, w.deps);
      assert.equal(outcome, "partial");
      assert.equal(w.store.config.checks.interval_sec, 900, "the main config was not stored");
    });

    it("the message names the network and the reason", async () => {
      const w = failingB();
      await saveDraft(w.form, w.store, w.deps);
      assert.ok(w.form.message.text.startsWith("Saved, but some networks did not accept it:"), w.form.message.text);
      assert.ok(w.form.message.text.includes("\"my devices\": boom"), w.form.message.text);
      assert.equal(w.form.message.cls, "field-error-text");
    });

    it("the shared field stays in the draft", async () => {
      const w = failingB();
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.draft), { "checks.interval_sec": "900" });
    });

    it("the second Save sends both PUTs again", async () => {
      const w = failingB();
      await saveDraft(w.form, w.store, w.deps);
      w.calls.length = 0;
      w.replies[`PUT ${BPATH}`] = [null];
      const outcome = await saveDraft(w.form, w.store, w.deps);
      assert.equal(outcome, "saved");
      assert.deepEqual(w.calls.map(callLine), ["PUT /api/config", `PUT ${BPATH}`]);
      assert.deepEqual(plain(w.form.draft), {});
    });
  });
});

describe("restartTargets over the store", () => {
  it("restart_slots is empty, the main network has restart_pending: [\"main\"]", () => {
    const w = setupNets("one");
    w.store.restartSlots = [];
    w.store.configMeta = { read_only: [], restart_fields: [], restart_pending: ["socks.port"], cli_overrides: [] };
    assert.deepEqual(restartTargets(w.store), ["main"]);
  });
});

describe("restartNode over several networks", () => {
  describe("restart_slots lists two networks", () => {
    it("POSTs restart to each, the main one first", async () => {
      const w = setupNets("two");
      w.store.restartSlots = [B, "main"];
      const outcome = await restartNode(w.form, w.store, w.deps);
      assert.equal(outcome, "restarted");
      assert.deepEqual(w.calls.map(callLine), ["POST /api/actions/restart", `POST /api/net/${B}/actions/restart`]);
    });

    it("the other network refuses: failed, with the network named in the message", async () => {
      const w = setupNets("two", { [`POST /api/net/${B}/actions/restart`]: [() => { throw new ApiError(500, "http", "boom"); }] });
      w.store.restartSlots = ["main", B];
      const outcome = await restartNode(w.form, w.store, w.deps);
      assert.equal(outcome, "failed");
      assert.ok(w.form.message.text.includes("\"my devices\": boom"), w.form.message.text);
    });

    it("the main network refuses: the other one is still asked", async () => {
      const w = setupNets("two", { "POST /api/actions/restart": [() => { throw new ApiError(500, "http", "boom"); }] });
      w.store.restartSlots = ["main", B];
      assert.equal(await restartNode(w.form, w.store, w.deps), "failed");
      assert.equal(w.calls.length, 2);
    });
  });

  describe("only the other network is listed", () => {
    it("the main network is not restarted", async () => {
      const w = setupNets("two");
      w.store.restartSlots = [B];
      await restartNode(w.form, w.store, w.deps);
      assert.deepEqual(w.calls.map(callLine), [`POST /api/net/${B}/actions/restart`]);
    });
  });

  describe("restart_slots is empty, restart_pending is set", () => {
    it("one POST /api/actions/restart", async () => {
      const w = setupNets("one");
      w.store.restartSlots = [];
      w.store.configMeta = { read_only: [], restart_fields: [], restart_pending: ["socks.port"], cli_overrides: [] };
      assert.equal(await restartNode(w.form, w.store, w.deps), "restarted");
      assert.deepEqual(w.calls.map(callLine), ["POST /api/actions/restart"]);
    });
  });
});

/** The PUT hangs until `reply.resolve()` is called; `socks.port` — "after restart". */
function setup() {
  const store = createStore();
  store.config = {
    socks: { enabled: true, port: 1080 },
    checks: { enabled: true, provider_dns: null },
    autostart: { available: true, reason: "", enabled: false },
  };
  store.configRev = 5;
  store.configMeta = { read_only: [], restart_fields: [LATER], restart_pending: [], cli_overrides: [] };
  store.configFile = null;
  const reply = deferred();
  const api = async (method, path, body) => {
    await reply.promise;
    return { config: structuredClone(body.config), rev: body.rev + 1, restart_fields: [LATER] };
  };
  const deps = { api, busTarget: new EventTarget(), setTimer: () => 1, clearTimer: () => {} };
  return { store, form: createSettingsForm(), deps, reply };
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => ((resolve = res), (reject = rej)));
  return { promise, resolve, reject };
}

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

const B = "k7q2xa";
const C = "m3n4pq";
const BPATH = `/api/net/${B}/config`;
const CPATH = `/api/net/${C}/config`;

const MAIN_CONFIG = {
  socks: { enabled: true, port: 1080, access: "localhost" },
  checks: { enabled: true, interval_sec: 300, doh_server: "dns.example:443", provider_dns: null },
  probe: { interval_sec: 600 },
  autostart: { available: true, reason: "", enabled: false },
};
// A different socks port and interval: the other network's own values must survive a shared write.
const SLOT_CONFIG = {
  ...MAIN_CONFIG,
  socks: { enabled: true, port: 1081, access: "localhost" },
  checks: { ...MAIN_CONFIG.checks, interval_sec: 300 },
};

function brief(id, over = {}) {
  return {
    id, label: null, id_hex: null, status: "running", exit_enabled: false, proxy: id === "main",
    route_state: "direct", exits: [], members_voting: 1, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0, ...over,
  };
}

function netsOf(layout, { bJoining = false } = {}) {
  const main = brief("main", { label: "friends", id_hex: "aa00bb" });
  const b = brief(B, { label: "my devices", id_hex: "3f2a00", joining: bJoining });
  const c = brief(C, { label: "work", id_hex: "9c1d00" });
  if (layout === "legacy") return [];
  if (layout === "one") return [main];
  if (layout === "lone") return [{ ...main, id_hex: null }, b];
  return layout === "three" ? [main, b, c] : [main, b];
}

const callLine = (c) => `${c.method} ${c.path}`;

/**
 * A store with the layout of nets (`legacy` — no overview, `one`, `two`, `three`) and an `api` that records calls.
 * `replies` — per `"METHOD /path"` a list of steps used in order (the last one repeats): a function throws or
 * returns the reply; `null` — the default echo of the written config.
 */
function setupNets(layout, replies = {}, opts = {}) {
  const store = createStore();
  store.config = structuredClone(MAIN_CONFIG);
  store.configRev = 5;
  store.configMeta = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
  store.configFile = null;
  store.nets = netsOf(layout, opts);
  for (const n of store.nets) {
    const config = n.id === "main" ? structuredClone(MAIN_CONFIG) : structuredClone(SLOT_CONFIG);
    store.slots[n.id] = { id: n.id, brief: n, state: {}, members: null, config, configRev: 11, configMeta: null, bootId: null };
  }
  const calls = [];
  const api = async (method, path, body) => {
    calls.push({ method, path, body: body === undefined ? undefined : structuredClone(body) });
    const queue = w.replies[`${method} ${path}`] || [null];
    const step = queue.length > 1 ? queue.shift() : queue[0];
    if (step) return step(body);
    return method === "PUT" ? { config: structuredClone(body.config), rev: body.rev + 1 } : {};
  };
  const deps = { api, busTarget: new EventTarget(), setTimer: () => 1, clearTimer: () => {} };
  const w = { store, form: createSettingsForm(), deps, calls, replies };
  return w;
}

/** Two networks, the other one rejects the write, the shared interval is edited. */
function failingB() {
  const w = setupNets("two", { [`PUT ${BPATH}`]: [() => { throw new ApiError(500, "http", "boom"); }] });
  editField(w.form, "checks.interval_sec", "900");
  return w;
}
