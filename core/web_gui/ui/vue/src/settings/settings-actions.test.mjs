/* Tests for the Settings form actions over `form` and the store with mock `api`, `confirm`, `setToken`,
 * the timer and `busTarget`; `structuredClone` is the real one. The output reads as a phrase:
 * `saveDraft > success > draft_empty`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import { EV_CHANGED } from "../core/bridge.ts";
import { createStore } from "../core/store.ts";
import { CONFIRM_STOP_TEXT } from "../common/pac-texts.ts";
import { NET_NAME } from "../common/words.ts";
import {
  createSettingsForm,
  editField,
  resetDraft,
  saveDraft,
  reloadFresh,
  restartNode,
  onPendingEmpty,
  disposeForm,
  stopNode,
  rotateToken,
  toggleToken,
} from "./settings-actions.ts";
import { saveBarView, RESTART_STARTED } from "./settings-view.ts";

describe("createSettingsForm / editField / resetDraft", () => {
  it("createSettingsForm — a clean form", () => {
    // The `token` and `timer` fields are not checked.
    const { token, timer, ...planned } = createSettingsForm();
    assert.deepEqual(plain(planned), {
      draft: {},
      errors: {},
      conflict: null,
      message: { text: "", cls: "hint" },
      busy: false,
      restartRequested: false,
      hideFileFieldErrors: false,
      expertOpen: false,
      tokenRevealed: false,
    });
  });

  it("editField writes the raw value to the draft", () => {
    const form = createSettingsForm();
    editField(form, "socks.port", "1081");
    editField(form, "socks.enabled", false);
    assert.deepEqual(plain(form.draft), { "socks.port": "1081", "socks.enabled": false });
  });

  it("resetDraft resets the draft, errors and conflict, but not the expanded state and not the message about something else", () => {
    const form = createSettingsForm();
    editField(form, "socks.port", "1");
    form.errors = { "socks.port": "bad" };
    form.conflict = { rev: 3 };
    form.expertOpen = true;
    resetDraft(form);
    assert.deepEqual(plain(form.draft), {});
    assert.deepEqual(plain(form.errors), {});
    assert.equal(form.conflict, null);
    assert.equal(form.expertOpen, true);
  });
});

describe("saveDraft", () => {
  describe("success", () => {
    it("PUT {rev, config} with the parsed socks.port value 1081", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      const res = await saveDraft(w.form, w.store, w.deps);
      assert.equal(res, "saved");
      assert.equal(w.calls.length, 1);
      assert.equal(w.calls[0].method, "PUT");
      assert.equal(w.calls[0].path, "/api/config");
      assert.equal(w.calls[0].body.rev, 5);
      assert.equal(w.calls[0].body.config.socks.port, 1081);
      assert.equal(w.calls[0].body.config.socks.enabled, true, "the rest of the config went out unchanged");
    });

    it("draft is empty, errors cleared, busy released, file errors hidden", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      w.form.errors = { "socks.port": "old" };
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.draft), {});
      assert.deepEqual(plain(w.form.errors), {});
      assert.equal(w.form.busy, false);
      assert.equal(w.form.hideFileFieldErrors, true);
      assert.equal(w.form.restartRequested, false);
    });

    it("the text \"Saved\", class saved; a field \"after restart\" — the long text", async () => {
      const w = setup({ script: [put200({ restart_fields: ["socks.port"] })] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.message), { text: "Saved. Takes effect after Mesh Town restarts.", cls: "saved" });
    });

    it("a field outside \"after restart\" — just \"Saved\"", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Saved");
    });

    it("one meshtown:changed {what:'config'}, the store updated from the response", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(w.events, [{ what: "config" }]);
      assert.equal(w.store.configRev, 6);
      assert.equal(w.store.config.socks.port, 1081);
    });
  });

  describe("\"Saved\" timer", () => {
    it("after 4000 ms the text is cleared", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.timers.length, 1);
      assert.equal(w.timers[0].ms, 4000);
      w.timers[0].fn();
      assert.equal(w.form.message.text, "");
    });

    it("with a non-empty restart_pending the text stays", async () => {
      const w = setup({ script: [put200({ restart_fields: ["socks.port"], restart_pending: ["socks.port"] })] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      const before = w.form.message.text;
      w.timers[0].fn();
      assert.equal(w.form.message.text, before);
      assert.match(before, /after Mesh Town restarts/);
    });

    it("the text has changed to another one — the timer does not touch it", async () => {
      const w = setup({ script: [put200()] });
      editField(w.form, "checks.enabled", false);
      await saveDraft(w.form, w.store, w.deps);
      w.form.message = { text: "other", cls: "hint" };
      w.timers[0].fn();
      assert.equal(w.form.message.text, "other");
    });
  });

  describe("skip", () => {
    it("the file is not writable — \"skipped\", 0 requests", async () => {
      const w = setup({ script: [] });
      w.store.configFile = { writable: false, message: "read only" };
      editField(w.form, "socks.port", "1081");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      assert.equal(w.calls.length, 0);
    });

    it("busy — \"skipped\", 0 requests", async () => {
      const w = setup({ script: [] });
      w.form.busy = true;
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      assert.equal(w.calls.length, 0);
    });

    it("a repeated call while the first is in flight — \"skipped\", one request", async () => {
      const gate = deferred();
      const w = setup({ script: [() => gate.promise] });
      editField(w.form, "socks.port", "1081");
      const first = saveDraft(w.form, w.store, w.deps);
      await tick();
      assert.equal(w.form.busy, true);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "skipped");
      gate.resolve(replyBody());
      assert.equal(await first, "saved");
      assert.equal(w.calls.length, 1);
    });
  });

  describe("409 stale_rev", () => {
    it("\"conflict\", conflict.rev from the body, draft intact, no events, store untouched", async () => {
      const w = setup({ script: [fail("stale_rev", 409, { rev: 9, config: {} })] });
      editField(w.form, "socks.port", "1081");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "conflict");
      assert.equal(w.form.conflict.rev, 9);
      assert.deepEqual(plain(w.form.draft), { "socks.port": "1081" });
      assert.deepEqual(w.events, []);
      assert.equal(w.store.configRev, 5);
      assert.equal(w.form.busy, false);
      assert.equal(w.calls.length, 1, "there must be no auto-retry");
    });

    it("retry with form.conflict.rev — the second PUT goes out with this rev, the conflict is cleared", async () => {
      const w = setup({ script: [fail("stale_rev", 409, { rev: 9, config: {} }), put200()] });
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      const res = await saveDraft(w.form, w.store, w.deps, w.form.conflict.rev);
      assert.equal(res, "saved");
      assert.equal(w.calls[1].body.rev, 9);
      assert.equal(w.form.conflict, null);
      assert.deepEqual(w.events, [{ what: "config" }]);
    });
  });

  describe("422", () => {
    it("checks.provider_dns — errors with the path, a general text, \"For specialists\" expanded", async () => {
      const fields = [{ path: "checks.provider_dns", message: "host:port required" }];
      const w = setup({ script: [fail("invalid", 422, { fields })] });
      editField(w.form, "checks.provider_dns", "junk");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
      assert.deepEqual(plain(w.form.errors), { "checks.provider_dns": "host:port required" });
      assert.deepEqual(plain(w.form.message), { text: "There are errors in the fields — see the notes below.", cls: "field-error-text" });
      assert.equal(w.form.expertOpen, true);
      assert.deepEqual(plain(w.form.draft), { "checks.provider_dns": "junk" }, "draft not lost");
      assert.deepEqual(w.events, []);
    });

    it("an error in a \"Main\" field — \"For specialists\" is not expanded", async () => {
      const fields = [{ path: "socks.port", message: "in use" }];
      const w = setup({ script: [fail("invalid", 422, { fields })] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.expertOpen, false);
    });

    it("not_available — also \"invalid\"", async () => {
      const fields = [{ path: "autostart.enabled", message: "unavailable" }];
      const w = setup({ script: [fail("not_available", 409, { fields })] });
      editField(w.form, "autostart.enabled", true);
      assert.equal(await saveDraft(w.form, w.store, w.deps), "invalid");
      assert.equal(w.form.errors["autostart.enabled"], "unavailable");
    });

    it("a new attempt replaces the previous errors", async () => {
      const w = setup({ script: [fail("invalid", 422, { fields: [{ path: "socks.port", message: "in use" }] }), put200()] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      editField(w.form, "socks.port", "1081");
      await saveDraft(w.form, w.store, w.deps);
      assert.deepEqual(plain(w.form.errors), {});
    });
  });

  describe("other failures", () => {
    it("config_write_failed — text from the body or a fallback", async () => {
      const w = setup({ script: [fail("config_write_failed", 500, { message: "disk full" }), fail("config_write_failed", 500, null)] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.deepEqual(plain(w.form.message), { text: "Couldn't save: disk full", cls: "field-error-text" });
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Couldn't write the settings file");
    });

    it("500 — the body text; busy released; draft intact; no events", async () => {
      const w = setup({ script: [fail("boom", 500, { message: "node failure" })] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.deepEqual(plain(w.form.message), { text: "Couldn't save: node failure", cls: "field-error-text" });
      assert.equal(w.form.busy, false);
      assert.deepEqual(plain(w.form.draft), { "socks.port": "1" });
      assert.deepEqual(w.events, []);
    });

    it("network — the core error text", async () => {
      const w = setup({ script: [() => { throw new ApiError(0, "network", API_ERROR_TEXT.network); }] });
      editField(w.form, "socks.port", "1");
      assert.equal(await saveDraft(w.form, w.store, w.deps), "failed");
      assert.match(w.form.message.text, /^Couldn't save: /);
      assert.equal(w.form.busy, false);
    });

    it("an error without text — \"Couldn't save — try again\"", async () => {
      const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x", status: 500, body: null }); }] });
      editField(w.form, "socks.port", "1");
      await saveDraft(w.form, w.store, w.deps);
      assert.equal(w.form.message.text, "Couldn't save — try again");
    });
  });
});

describe("reloadFresh", () => {
  it("draft and conflict are empty, one meshtown:changed {what:'config'}, no GET of its own", () => {
    const w = setup({ script: [] });
    editField(w.form, "socks.port", "1");
    w.form.conflict = { rev: 9 };
    reloadFresh(w.form, w.deps);
    assert.deepEqual(plain(w.form.draft), {});
    assert.equal(w.form.conflict, null);
    assert.deepEqual(w.events, [{ what: "config" }]);
    assert.equal(w.calls.length, 0);
  });
});

describe("disposeForm", () => {
  it("clears the message timer and zeroes form.timer", () => {
    const cleared = [];
    const form = createSettingsForm();
    form.timer = 7;
    disposeForm(form, { clearTimer: (id) => cleared.push(id) });
    assert.deepEqual(cleared, [7]);
    assert.equal(form.timer, null);
  });

  it("without a timer it clears nothing", () => {
    const cleared = [];
    disposeForm(createSettingsForm(), { clearTimer: (id) => cleared.push(id) });
    assert.deepEqual(cleared, []);
  });
});

describe("restartNode", () => {
  describe("result", () => {
    it("restarted / failed / skipped", async () => {
      const ok = setup({ script: [() => ({})] });
      assert.equal(await restartNode(ok.form, ok.deps), "restarted");
      const bad = setup({ script: [() => { throw new ApiError(409, "busy", "x"); }] });
      assert.equal(await restartNode(bad.form, bad.deps), "failed");
      const busy = setup({ script: [] });
      busy.form.busy = true;
      assert.equal(await restartNode(busy.form, busy.deps), "skipped");
    });
  });

  describe("success", () => {
    it("POST /api/actions/restart, restartRequested, the text \"Mesh Town is restarting…\" with class saved", async () => {
      const w = setup({ script: [() => ({ restart: true })] });
      await restartNode(w.form, w.deps);
      assert.equal(w.calls[0].method, "POST");
      assert.equal(w.calls[0].path, "/api/actions/restart");
      assert.equal(w.form.restartRequested, true);
      assert.deepEqual(plain(w.form.message), { text: RESTART_STARTED, cls: "saved" });
    });
  });

  describe("failure", () => {
    it("the node's error text, restartRequested not set", async () => {
      const w = setup({ script: [() => { throw new ApiError(409, "busy", "node is starting"); }] });
      await restartNode(w.form, w.deps);
      assert.equal(w.form.message.text, "Couldn't restart the node: node is starting");
      assert.equal(w.form.message.cls, "field-error-text");
      assert.equal(w.form.restartRequested, false);
    });

    it("an error without text — \"Couldn't restart the node — try again\"", async () => {
      const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x" }); }] });
      await restartNode(w.form, w.deps);
      assert.equal(w.form.message.text, "Couldn't restart the node — try again");
    });
  });

  describe("response after restart_pending has emptied", () => {
    it("the bar does not show \"Mesh Town is restarting\"", async () => {
      const w = setup({ script: [() => ({ restart: true })] });
      w.store.configMeta = meta({ restart_pending: [] });
      await restartNode(w.form, w.deps);
      const view = saveBarView({
        dirty: false,
        meta: w.store.configMeta,
        configFile: null,
        requested: w.form.restartRequested,
        busy: w.form.busy,
        message: w.form.message,
      });
      assert.equal(view.messageShown, "");
    });
  });
});

describe("onPendingEmpty", () => {
  it("resets restartRequested and clears the text \"Mesh Town is restarting\"", () => {
    const form = createSettingsForm();
    form.restartRequested = true;
    form.message = { text: RESTART_STARTED, cls: "saved" };
    onPendingEmpty(form);
    assert.equal(form.restartRequested, false);
    assert.equal(form.message.text, "");
  });

  it("does not touch someone else's text (\"Saved. Takes effect…\")", () => {
    const form = createSettingsForm();
    form.restartRequested = true;
    form.message = { text: "Saved. Takes effect after Mesh Town restarts.", cls: "saved" };
    onPendingEmpty(form);
    assert.equal(form.message.text, "Saved. Takes effect after Mesh Town restarts.");
  });
});

describe("stopNode", () => {
  it("confirm → false: 0 requests, no events", async () => {
    const w = setup({ script: [], confirm: false });
    await stopNode(w.form, w.deps);
    assert.equal(w.calls.length, 0);
    assert.equal(w.form.message.text, "");
    assert.deepEqual(w.events, []);
  });

  it("confirm asks about stopping the node", async () => {
    const w = setup({ script: [], confirm: false });
    await stopNode(w.form, w.deps);
    assert.ok(w.confirms[0].startsWith("Stop " + NET_NAME + "?"));
  });

  it("confirm: the full CONFIRM_STOP_TEXT text — sites in the browser don't open", async () => {
    const w = setup({ script: [], confirm: false });
    await stopNode(w.form, w.deps);
    assert.equal(w.confirms.length, 1);
    assert.equal(w.confirms[0], CONFIRM_STOP_TEXT);
    assert.equal(
      w.confirms[0],
      `Stop ${NET_NAME}? While it is stopped, sites in the browser don't open (local addresses do); after it starts, they come back in about 15 seconds.`,
    );
  });

  it("success: POST /api/actions/stop, events stopping then stopped", async () => {
    const w = setup({ script: [() => ({})] });
    assert.equal(await stopNode(w.form, w.deps), true);
    assert.equal(w.calls[0].method, "POST");
    assert.equal(w.calls[0].path, "/api/actions/stop");
    assert.deepEqual(w.events, [{ what: "stopping" }, { what: "stopped" }]);
  });

  it("confirmed: the stopping event goes out before the stop request", async () => {
    // given: events and requests recorded into one list
    const w = setup({ script: [() => ({})] });
    const order = [];
    w.target.addEventListener(EV_CHANGED, (ev) => order.push("event:" + ev.detail.what));
    const api = w.deps.api;
    w.deps.api = (method, path, body) => (order.push("api:" + method + " " + path), api(method, path, body));
    // when
    await stopNode(w.form, w.deps);
    // then
    assert.ok(order.indexOf("event:stopping") >= 0, "no stopping event");
    assert.ok(
      order.indexOf("event:stopping") < order.indexOf("api:POST /api/actions/stop"),
      "stopping must precede the request, got " + order.join(", "),
    );
  });

  it("declined: no events and no requests", async () => {
    const w = setup({ script: [], confirm: false });
    assert.equal(await stopNode(w.form, w.deps), false);
    assert.deepEqual(w.events, []);
    assert.equal(w.calls.length, 0);
  });

  it("node failure — the line \"Couldn't stop the node: …\", events stopping then state, no stopped", async () => {
    const w = setup({ script: [() => { throw new ApiError(500, "x", "Node failure"); }] });
    assert.equal(await stopNode(w.form, w.deps), false);
    assert.deepEqual(w.events, [{ what: "stopping" }, { what: "state" }]);
    assert.deepEqual(plain(w.form.message), { text: "Couldn't stop the node: node failure", cls: "field-error-text" });
  });

  it("failure without text — \"— try again\"", async () => {
    const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x" }); }] });
    await stopNode(w.form, w.deps);
    assert.equal(w.form.message.text, "Couldn't stop the node — try again");
  });
});

describe("rotateToken", () => {
  it("confirm → false: 0 requests, token not written", async () => {
    const w = setup({ script: [], confirm: false });
    await rotateToken(w.form, w.deps);
    assert.equal(w.calls.length, 0);
    assert.deepEqual(w.tokens, []);
  });

  it("confirm warns that old links will stop working", async () => {
    const w = setup({ script: [], confirm: false });
    await rotateToken(w.form, w.deps);
    assert.equal(w.confirms[0], "Old links to this page will stop working immediately; this tab will get the new token by itself. Continue?");
  });

  it("success: POST /api/token/rotate, setToken with the new token, token revealed, the text \"New token…\"", async () => {
    const w = setup({ script: [() => ({ token: "new-tok" })] });
    await rotateToken(w.form, w.deps);
    assert.equal(w.calls[0].path, "/api/token/rotate");
    assert.deepEqual(w.tokens, ["new-tok"]);
    assert.equal(w.form.tokenRevealed, true);
    assert.deepEqual(plain(w.form.message), { text: "The new token has been issued and applied", cls: "saved" });
  });

  it("rotation failure: token unchanged, nothing written, token not revealed", async () => {
    const w = setup({ script: [() => { throw new ApiError(500, "x", "token not changed: disk full"); }] });
    assert.equal(await rotateToken(w.form, w.deps), false);
    assert.deepEqual(w.tokens, []);
    assert.equal(w.form.tokenRevealed, false);
    assert.deepEqual(plain(w.form.message), { text: "Token not changed: disk full", cls: "field-error-text" });
  });

  it("failure without text — \"Couldn't change the token — try again\"", async () => {
    const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x" }); }] });
    await rotateToken(w.form, w.deps);
    assert.equal(w.form.message.text, "Couldn't change the token — try again");
  });

  it("a response without a token — nothing changes", async () => {
    const w = setup({ script: [() => ({})] });
    await rotateToken(w.form, w.deps);
    assert.deepEqual(w.tokens, []);
    assert.equal(w.form.tokenRevealed, false);
    assert.equal(w.form.message.text, "Couldn't change the token — try again");
  });
});

describe("toggleToken", () => {
  it("shows and hides", () => {
    const form = createSettingsForm();
    const deps = { getToken: () => "tok" };
    toggleToken(form, deps);
    assert.equal(form.tokenRevealed, true);
    toggleToken(form, deps);
    assert.equal(form.tokenRevealed, false);
  });
});

function meta(over = {}) {
  return { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [], ...over };
}

function configBody() {
  return {
    socks: { enabled: true, port: 1080 },
    checks: { enabled: true, provider_dns: null },
    autostart: { available: true, reason: "", enabled: false },
  };
}

// The server's response to PUT: the config with the port edit applied, rev 6.
function replyBody(over = {}) {
  const config = configBody();
  config.socks.port = 1081;
  return { config, rev: 6, ...over };
}

function put200(over = {}) {
  return () => replyBody(over);
}

function fail(code, status, body) {
  return () => {
    throw new ApiError(status, code, (body && body.message) || code, body);
  };
}

/** `script` — one response per `api` call; `confirm` — what the confirmation dialog answers. */
function setup({ script, confirm = true }) {
  const store = createStore();
  store.config = configBody();
  store.configRev = 5;
  store.configMeta = meta();
  store.configFile = null;
  const form = createSettingsForm();
  const target = new EventTarget();
  const events = [];
  target.addEventListener(EV_CHANGED, (ev) => events.push(ev.detail));
  const calls = [];
  const api = async (method, path, body) => {
    calls.push({ method, path, body: body === undefined ? undefined : structuredClone(body) });
    const step = script[calls.length - 1];
    assert.ok(step, `unexpected request ${method} ${path}`);
    return step();
  };
  const timers = [];
  const tokens = [];
  const confirms = [];
  const deps = {
    api,
    busTarget: target,
    confirm: (text) => {
      confirms.push(text);
      return confirm;
    },
    setToken: (t) => tokens.push(t),
    setTimer: (fn, ms) => timers.push({ fn, ms }) && timers.length,
    clearTimer: () => {},
  };
  return { store, form, deps, calls, events, timers, tokens, confirms, target };
}

function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

const tick = () => new Promise((r) => setImmediate(r));

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}
