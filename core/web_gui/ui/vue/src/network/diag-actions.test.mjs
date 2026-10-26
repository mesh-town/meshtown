/* Actions of "Diagnostics" over `form`, `key` and the store with fake `api`, `clipboard`, timer.
 * The output reads as a phrase: `toggleKey > network_changed_in_flight > stale`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  createDiagForm,
  createKeyState,
  copySummary,
  copySelfAddresses,
  addPeerAddresses,
  toggleKey,
  resetKeyIfNetChanged,
} from "./diag-actions.ts";
import { summaryLogPath } from "./diag-summary.ts";
import { NET_NAME } from "../common/words.ts";

const SECRET_KEY = "mesh1-FULLSECRETKEYVALUE";

describe("createDiagForm / createKeyState", () => {
  it("clean form", () => {
    assert.deepEqual(createDiagForm(), {
      specOpen: false,
      summary: { busy: false, status: "", fallback: "" },
      peerAddr: { draft: "", busy: false, result: "" },
      copyStatus: "",
    });
  });

  it("clean key state", () => {
    assert.deepEqual(createKeyState(), { shown: false, full: null, netId: null, busy: false, error: "" });
  });
});

describe("copySummary", () => {
  it("success: one GET of the log, the clipboard got the summary, the status \"Summary copied\"", async () => {
    const w = setup();
    const outcome = await copySummary(w.form, w.store, w.deps);
    assert.equal(outcome, "copied");
    assert.deepEqual(w.calls, [["GET", summaryLogPath(w.store.state)]]);
    assert.equal(w.written.length, 1);
    assert.ok(w.written[0].startsWith("Mesh Town 1.2.3"), w.written[0]);
    assert.equal(w.form.summary.status, "Summary copied");
    assert.equal(w.form.summary.fallback, "");
  });

  it("success: the status fades after 4000 ms by the timer", async () => {
    const w = setup();
    await copySummary(w.form, w.store, w.deps);
    assert.equal(w.timers.length, 1);
    assert.equal(w.timers[0].ms, 4000);
    w.timers[0].fn();
    assert.equal(w.form.summary.status, "");
  });

  it("no clipboard (clipboard null) — \"fallback\", the summary text in form.summary.fallback", async () => {
    const w = setup({ clipboard: null });
    assert.equal(await copySummary(w.form, w.store, w.deps), "fallback");
    assert.ok(w.form.summary.fallback.startsWith("Mesh Town 1.2.3"));
    assert.equal(w.form.summary.status, "Clipboard unavailable — select the summary below and copy it manually.");
  });

  it("writeText rejected — \"fallback\"", async () => {
    const w = setup({ clipboard: { writeText: async () => { throw new Error("denied"); } } });
    assert.equal(await copySummary(w.form, w.store, w.deps), "fallback");
    assert.ok(w.form.summary.fallback.length > 0);
  });

  it("request in flight — the second call is \"skipped\", one request", async () => {
    const w = setup();
    const gate = deferred();
    w.deps.api = async (m, p) => { w.calls.push([m, p]); return gate.promise; };
    const first = copySummary(w.form, w.store, w.deps);
    assert.equal(await copySummary(w.form, w.store, w.deps), "skipped");
    gate.resolve({ lines: [] });
    assert.equal(await first, "copied");
    assert.equal(w.calls.length, 1);
  });

  it("the log answered 500 — a summary without the log, with \"log unavailable: …\"", async () => {
    const w = setup({ api: async () => { throw new Error("The node responded with an error 500"); } });
    assert.equal(await copySummary(w.form, w.store, w.deps), "copied");
    assert.match(w.written[0], /log unavailable: The node responded with an error 500/);
    assert.match(w.written[0], /✓ You're in the network/);
  });

  it("the log refused without a message — \"log unavailable: no answer\"", async () => {
    const w = setup({ api: async () => { throw {}; } });
    await copySummary(w.form, w.store, w.deps);
    assert.match(w.written[0], /log unavailable: no answer/);
  });

  it("no log in the snapshot — 0 requests, \"log unavailable: no data\"", async () => {
    const w = setup();
    delete w.store.state.log;
    await copySummary(w.form, w.store, w.deps);
    assert.equal(w.calls.length, 0);
    assert.match(w.written[0], /log unavailable: no data/);
  });

  it("busy is cleared after completion", async () => {
    const w = setup();
    await copySummary(w.form, w.store, w.deps);
    assert.equal(w.form.summary.busy, false);
  });
});

describe("copySelfAddresses", () => {
  it("the clipboard got the addresses line by line, the status \"Copied\"", async () => {
    const w = setup();
    assert.equal(await copySelfAddresses(w.form, w.store, w.deps), "copied");
    assert.deepEqual(w.written, ["1.1.1.1:1\n2.2.2.2:2"]);
    assert.equal(w.form.copyStatus, "Copied");
  });

  it("no clipboard — \"fallback\"", async () => {
    const w = setup({ clipboard: null });
    assert.equal(await copySelfAddresses(w.form, w.store, w.deps), "fallback");
    assert.equal(w.form.copyStatus, "Clipboard unavailable — select the addresses and copy them manually.");
  });
});

describe("addPeerAddresses", () => {
  it("empty field and spaces — \"empty\", no requests", async () => {
    const w = setup();
    for (const draft of ["", "   \n "]) {
      w.form.peerAddr.draft = draft;
      assert.equal(await addPeerAddresses(w.form, w.deps), "empty");
    }
    assert.equal(w.calls.length, 0);
    assert.equal(w.form.peerAddr.result, "Paste the member's addresses — one per line");
  });

  it("success: POST {text}, the field is empty, the result as text", async () => {
    const w = setup({ api: async (m, p, b) => (w.calls.push([m, p, b]), { added: 2, rejected: [{ line: 2, message: "bad" }] }) });
    w.form.peerAddr.draft = "a:1\nb";
    assert.equal(await addPeerAddresses(w.form, w.deps), "added");
    assert.deepEqual(w.calls, [["POST", "/api/peer-addresses", { text: "a:1\nb" }]]);
    assert.equal(w.form.peerAddr.draft, "");
    assert.equal(w.form.peerAddr.result, "Addresses added: 2, rejected: 1 (2: bad)");
  });

  it("error: the field is intact, the error text in the result", async () => {
    const w = setup({ api: async () => { throw new Error("The node is not responding"); } });
    w.form.peerAddr.draft = "a:1";
    assert.equal(await addPeerAddresses(w.form, w.deps), "failed");
    assert.equal(w.form.peerAddr.draft, "a:1");
    assert.equal(w.form.peerAddr.result, "Couldn't add the addresses: the node is not responding");
    assert.equal(w.form.peerAddr.busy, false);
  });
});

describe("toggleKey > show", () => {
  it("GET /api/network/key, full from the response, netId — the network's id_hex", async () => {
    const w = setup();
    assert.equal(await toggleKey(w.key, w.store, w.deps), "shown");
    assert.deepEqual(w.calls, [["GET", "/api/network/key"]]);
    assert.equal(w.key.full, SECRET_KEY);
    assert.equal(w.key.netId, "id-aaaa");
    assert.equal(w.key.shown, true);
    assert.equal(w.key.busy, false);
  });

  it("repeat — \"hidden\" without a request, the key is forgotten", async () => {
    const w = setup();
    await toggleKey(w.key, w.store, w.deps);
    assert.equal(await toggleKey(w.key, w.store, w.deps), "hidden");
    assert.equal(w.calls.length, 1);
    assert.equal(w.key.shown, false);
    assert.equal(w.key.full, null);
  });

  it("request error — \"failed\", the error text, full null", async () => {
    const w = setup({ api: async () => { throw new Error("The node is not responding"); } });
    assert.equal(await toggleKey(w.key, w.store, w.deps), "failed");
    assert.equal(w.key.error, "Couldn't get the network key: the node is not responding");
    assert.equal(w.key.full, null);
    assert.equal(w.key.shown, false);
  });

  it("{key:null} — \"failed\", the error \"Mesh Town didn't return the network key…\"", async () => {
    const w = setup({ api: async () => ({ key: null }) });
    assert.equal(await toggleKey(w.key, w.store, w.deps), "failed");
    assert.equal(w.key.error, NET_NAME + " didn't return the network key — try again");
    assert.equal(w.key.full, null);
  });
});

describe("toggleKey > no network", () => {
  it("network null — \"failed\", a text about joining the network", async () => {
    const w = setup();
    w.store.state = { ...w.store.state, network: null };
    assert.equal(await toggleKey(w.key, w.store, w.deps), "failed");
    assert.equal(w.key.error, "You haven't joined the network — there is no key yet");
  });
});

describe("toggleKey > network changed in flight", () => {
  it("the store is replaced before resolve — \"stale\", the key is not shown, full null", async () => {
    const w = setup();
    const gate = deferred();
    w.deps.api = async () => gate.promise;
    const pending = toggleKey(w.key, w.store, w.deps);
    w.store.state = { ...w.store.state, network: { label: "other", masked: "mesh1-bb", id_hex: "id-bbbb" } };
    gate.resolve({ key: SECRET_KEY });
    assert.equal(await pending, "stale");
    assert.equal(w.key.full, null, "the key of the previous network must not remain");
    assert.equal(w.key.shown, false);
    assert.equal(w.key.netId, null);
    assert.equal(w.key.error, "The network changed — click \"Show\" again.");
  });

  it("the network disappeared in flight — also \"stale\"", async () => {
    const w = setup();
    const gate = deferred();
    w.deps.api = async () => gate.promise;
    const pending = toggleKey(w.key, w.store, w.deps);
    w.store.state = { ...w.store.state, network: null };
    gate.resolve({ key: SECRET_KEY });
    assert.equal(await pending, "stale");
    assert.equal(w.key.full, null);
  });
});

describe("resetKeyIfNetChanged", () => {
  it("the same network — the state is intact", () => {
    const key = { shown: true, full: SECRET_KEY, netId: "id-aaaa", busy: false, error: "" };
    resetKeyIfNetChanged(key, "id-aaaa");
    assert.equal(key.shown, true);
    assert.equal(key.full, SECRET_KEY);
  });

  it("another id_hex (same mask) — reset", () => {
    const key = { shown: true, full: SECRET_KEY, netId: "id-aaaa", busy: false, error: "" };
    resetKeyIfNetChanged(key, "id-bbbb");
    assert.deepEqual([key.shown, key.full, key.netId], [false, null, null]);
  });

  it("netId null (no network) — reset", () => {
    const key = { shown: true, full: SECRET_KEY, netId: "id-aaaa", busy: false, error: "" };
    resetKeyIfNetChanged(key, null);
    assert.deepEqual([key.shown, key.full, key.netId], [false, null, null]);
  });
});

describe("key secret", () => {
  it("after toggleKey neither the store nor the form contains the key", async () => {
    const w = setup();
    await copySummary(w.form, w.store, w.deps);
    await toggleKey(w.key, w.store, w.deps);
    assert.equal(w.key.full, SECRET_KEY, "the key lives in the key state");
    assert.ok(!JSON.stringify(w.store).includes(SECRET_KEY), "the key leaked into the store");
    assert.ok(!JSON.stringify(w.form).includes(SECRET_KEY), "the key leaked into the form");
  });
});

function deferred() {
  const d = {};
  d.promise = new Promise((resolve) => { d.resolve = resolve; });
  return d;
}

function setup(over = {}) {
  const w = { calls: [], written: [], timers: [] };
  w.store = {
    state: {
      node: { status: "online", uptime_sec: 60, version: "1.2.3" },
      network: { label: "home", masked: "mesh1-aa", id_hex: "id-aaaa" },
      transport: { relay: { connected: true }, dht: { joined: true }, self_addresses: ["1.1.1.1:1", "2.2.2.2:2"] },
      peers: [{ self: true, online: true }],
      route: { state: "direct", exits: [] },
      socks: { enabled: true, listen: "127.0.0.1:1080" },
      log: { tail: [], last_seq: 100 },
    },
  };
  w.form = createDiagForm();
  w.key = createKeyState();
  const clipboard = "clipboard" in over ? over.clipboard : { writeText: async (s) => void w.written.push(s) };
  const api = over.api || (async (m, p) => (w.calls.push([m, p]), p === "/api/network/key" ? { key: SECRET_KEY } : { lines: [] }));
  w.deps = {
    api,
    clipboard,
    setTimer: (fn, ms) => (w.timers.push({ fn, ms }), w.timers.length),
    clearTimer: () => {},
  };
  return w;
}
