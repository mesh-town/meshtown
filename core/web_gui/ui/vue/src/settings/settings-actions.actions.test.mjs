/* Tests for the Settings form lifecycle actions: reload, dispose, restart, stop and the token actions. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { EV_CHANGED } from "../core/bridge.ts";
import { CONFIRM_STOP_TEXT } from "../common/pac-texts.ts";
import { NET_NAME } from "../common/words.ts";
import {
  createSettingsForm,
  editField,
  reloadFresh,
  restartNode,
  onPendingEmpty,
  disposeForm,
  stopNode,
  rotateToken,
  toggleToken,
} from "./settings-actions.ts";
import { saveBarView, RESTART_STARTED } from "./settings-view.ts";
import { meta, restartable, setup, plain } from "./settings-actions-test-support.mjs";

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
      assert.equal(await restartNode(ok.form, ok.deps, restartable(ok.store)), "restarted");
      const bad = setup({ script: [() => { throw new ApiError(409, "busy", "x"); }] });
      assert.equal(await restartNode(bad.form, bad.deps, restartable(bad.store)), "failed");
      const busy = setup({ script: [] });
      busy.form.busy = true;
      assert.equal(await restartNode(busy.form, busy.deps, restartable(busy.store)), "skipped");
    });
  });

  describe("success", () => {
    it("POST /api/actions/restart, restartRequested, the text \"Mesh Town is restarting…\" with class saved", async () => {
      const w = setup({ script: [() => ({ restart: true })] });
      await restartNode(w.form, w.deps, restartable(w.store));
      assert.equal(w.calls[0].method, "POST");
      assert.equal(w.calls[0].path, "/api/actions/restart");
      assert.equal(w.form.restartRequested, true);
      assert.deepEqual(plain(w.form.message), { text: RESTART_STARTED, cls: "saved" });
    });
  });

  describe("failure", () => {
    it("the node's error text, restartRequested not set", async () => {
      const w = setup({ script: [() => { throw new ApiError(409, "busy", "node is starting"); }] });
      await restartNode(w.form, w.deps, restartable(w.store));
      assert.equal(w.form.message.text, "Couldn't restart the node: node is starting");
      assert.equal(w.form.message.cls, "field-error-text");
      assert.equal(w.form.restartRequested, false);
    });

    it("an error without text — \"Couldn't restart the node — try again\"", async () => {
      const w = setup({ script: [() => { throw Object.assign(new Error(""), { code: "x" }); }] });
      await restartNode(w.form, w.deps, restartable(w.store));
      assert.equal(w.form.message.text, "Couldn't restart the node — try again");
    });
  });

  describe("response after restart_pending has emptied", () => {
    it("the bar does not show \"Mesh Town is restarting\"", async () => {
      const w = setup({ script: [() => ({ restart: true })] });
      w.store.configMeta = meta({ restart_pending: [] });
      await restartNode(w.form, w.deps, restartable(w.store));
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
