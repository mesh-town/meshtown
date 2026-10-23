/* `use-ports-actions.ts`: editing the port lists via a fake `api` — the PUT body, the status, the form error. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { usePortsActions } from "./use-ports-actions.ts";
import { ADD_ERROR_NO_NODES } from "./ports-view.ts";

const PUB = [{ port: 80, name: "site", for: "all" }];

function setup({ ports = { published: PUB, opened: [] }, fail = null, delay = false } = {}) {
  const store = createStore();
  store.config = { ports, other: 1 };
  store.configRev = 3;
  const calls = [];
  const api = async (method, path, body) => {
    const sent = JSON.parse(JSON.stringify(body));
    calls.push({ method, path, body: sent });
    if (delay) await new Promise((resolve) => setTimeout(resolve, 5));
    if (fail) throw fail;
    return { config: sent.config, rev: body.rev + 1 };
  };
  const target = new EventTarget();
  const changed = [];
  target.addEventListener("meshtown:changed", (ev) => changed.push(ev.detail.what));
  const timers = [];
  const setTimer = (fn, ms) => timers.push({ fn, ms });
  const actions = usePortsActions(store, { api, busTarget: target, setTimer });
  return { store, calls, actions, changed, timers };
}

describe("publish", () => {
  it("success: PUT with the list, the status \"Saved\", true", async () => {
    const w = setup();
    assert.equal(await w.actions.publish(8080, " web "), true);
    assert.deepEqual(w.calls[0].body.config.ports.published, [...PUB, { port: 8080, name: "web", for: "all" }]);
    assert.equal(w.calls[0].body.config.other, 1);
    assert.deepEqual(w.actions.saveStatus.value, { text: "Saved", cls: "saved" });
    assert.equal(w.actions.addError.value, "");
  });

  it("a repeated port: a form error, no request", async () => {
    const w = setup();
    assert.equal(await w.actions.publish("80", ""), false);
    assert.equal(w.actions.addError.value, "Port 80 is already published.");
    assert.equal(w.calls.length, 0);
  });

  it("an empty port: a range error; the next successful input clears it", async () => {
    const w = setup();
    await w.actions.publish("", "");
    assert.equal(w.actions.addError.value, "A port is a number from 1 to 65535.");
    await w.actions.publish(81, "");
    assert.equal(w.actions.addError.value, "");
  });

  it("node refusal: false, a status with the error text", async () => {
    const w = setup({ fail: new ApiError(500, "boom", "failure") });
    assert.equal(await w.actions.publish(81, ""), false);
    assert.equal(w.actions.saveStatus.value.cls, "field-error-text");
    assert.match(w.actions.saveStatus.value.text, /^Couldn't save: /);
  });
});

describe("publish: node list", () => {
  it("an audience array — PUT with for: [id]", async () => {
    const w = setup();
    assert.equal(await w.actions.publish(8081, "web", ["aa"]), true);
    assert.deepEqual(w.calls[0].body.config.ports.published, [...PUB, { port: 8081, name: "web", for: ["aa"] }]);
  });

  it("an empty array — the ADD_ERROR_NO_NODES error, no request", async () => {
    const w = setup();
    assert.equal(await w.actions.publish(8081, "", []), false);
    assert.equal(w.actions.addError.value, ADD_ERROR_NO_NODES);
    assert.equal(w.calls.length, 0);
  });
});

describe("unpublish, open, close", () => {
  it("unpublish removes the port from ports.published", async () => {
    const w = setup();
    await w.actions.unpublish(80);
    assert.deepEqual(w.calls[0].body.config.ports.published, []);
  });

  it("open without a number writes the port plus 2000 (8080 → 10080), close removes the pair", async () => {
    const w = setup();
    await w.actions.openNetPort("main", "ab", 8080, "");
    assert.deepEqual(w.calls[0].body.config.ports.opened, [{ node: "ab", port: 8080, local: 10080 }]);
    await w.actions.closeNetPort("main", "ab", 8080);
    assert.deepEqual(w.calls[1].body.config.ports.opened, []);
  });

  it("open with its own number; no ports section in the config — does not fail", async () => {
    const w = setup({ ports: undefined });
    await w.actions.openNetPort("main", "ab", 8080, 9000);
    assert.deepEqual(w.calls[0].body.config.ports.opened, [{ node: "ab", port: 8080, local: 9000 }]);
  });
});

describe("snapshot refresh", () => {
  it("success of publish and unpublish: after meshtown:changed \"config\" comes \"state\"", async () => {
    const w = setup();
    await w.actions.publish(81, "");
    assert.deepEqual(w.changed, ["config", "state"]);
    await w.actions.unpublish(80);
    assert.deepEqual(w.changed, ["config", "state", "config", "state"]);
  });

  it("node refusal: \"state\" is not sent", async () => {
    const w = setup({ fail: new ApiError(500, "boom", "failure") });
    await w.actions.publish(81, "");
    assert.deepEqual(w.changed, []);
  });
});

describe("busy", () => {
  it("a second publish during saving — false without a request, busy is cleared; the list is not overwritten", async () => {
    const w = setup({ delay: true });
    const first = w.actions.publish(80 + 1, "");
    assert.equal(w.actions.saving.value, true);
    assert.equal(await w.actions.publish(82, ""), false);
    assert.equal(await first, true);
    assert.equal(w.calls.length, 1);
    assert.equal(w.actions.saving.value, false);
    assert.equal(await w.actions.publish(82, ""), true);
    assert.deepEqual(w.calls[1].body.config.ports.published.map((p) => p.port), [80, 81, 82]);
  });

  it("an api refusal does not leave busy: the next action goes through", async () => {
    const w = setup({ fail: new ApiError(500, "boom", "failure") });
    await w.actions.publish(81, "");
    assert.equal(w.actions.saving.value, false);
    await w.actions.unpublish(80);
    assert.equal(w.calls.length, 2);
  });

  it("no config: false, busy cleared", async () => {
    const w = setup();
    w.store.config = null;
    assert.equal(await w.actions.unpublish(80), false);
    assert.equal(w.actions.saving.value, false);
  });
});

describe("saving ports", () => {
  describe("success", () => {
    it("two polls: \"state\" at once, a 1200 ms timer, the second \"state\" after it", async () => {
      // given
      const w = setup();
      // when
      await w.actions.publish(81, "");
      // then
      assert.deepEqual(w.changed, ["config", "state"]);
      assert.deepEqual(w.timers.map((t) => t.ms), [1200]);
      w.timers[0].fn();
      assert.deepEqual(w.changed, ["config", "state", "state"]);
    });

    it("unpublish, open and close also set the second poll", async () => {
      const w = setup();
      await w.actions.unpublish(80);
      await w.actions.openNetPort("main", "ab", 8080, "");
      await w.actions.closeNetPort("main", "ab", 8080);
      assert.deepEqual(w.timers.map((t) => t.ms), [1200, 1200, 1200]);
    });
  });

  describe("refusal", () => {
    it("neither a poll nor a timer", async () => {
      // given
      const w = setup({ fail: new ApiError(500, "boom", "failure") });
      // when
      await w.actions.publish(81, "");
      // then
      assert.equal(w.changed.filter((what) => what === "state").length, 0, "no \"state\" events");
      assert.equal(w.timers.length, 0, "no timers");
    });
  });
});

describe("edit", () => {
  it("success: PUT with the replaced entry, editError is empty", async () => {
    const w = setup();
    assert.equal(await w.actions.edit(80, " main ", ["AA"]), true);
    assert.deepEqual(w.calls[0].body.config.ports.published, [{ port: 80, name: "main", for: ["AA"] }]);
    assert.equal(w.actions.editError.value, "");
    assert.equal(w.actions.addError.value, "");
  });

  it("an empty node list: the text in editError, addError untouched, no request", async () => {
    const w = setup();
    assert.equal(await w.actions.edit(80, "x", []), false);
    assert.equal(w.actions.editError.value, ADD_ERROR_NO_NODES);
    assert.equal(w.actions.addError.value, "");
    assert.equal(w.calls.length, 0);
  });

  it("no such port: an error, no request", async () => {
    const w = setup();
    assert.equal(await w.actions.edit(99, "x", "all"), false);
    assert.equal(w.actions.editError.value, "Port 99 is not published.");
    assert.equal(w.calls.length, 0);
  });
});

describe("edit: node refusal", () => {
  it("false, the refusal text in editError, the editor error is the only place it is shown", async () => {
    // given
    const w = setup({ fail: new ApiError(500, "boom", "disk is full") });
    // when
    const ok = await w.actions.edit(80, "main", "all");
    // then
    assert.equal(ok, false);
    assert.match(w.actions.editError.value, /^Couldn't save: /);
    assert.equal(w.actions.saveStatus.value.cls === "field-error-text", false, "the shared status shows the refusal");
    assert.equal(w.actions.saveStatus.value.text, "", "the shared status is not empty");
    assert.equal(w.actions.addError.value, "");
  });

  it("an invalid-field refusal (422) lands in editError as well", async () => {
    const body = { fields: [{ path: "ports.published", message: "Port 80 clashes with another service" }] };
    const w = setup({ fail: new ApiError(422, "invalid", "invalid", body) });
    assert.equal(await w.actions.edit(80, "main", "all"), false);
    assert.equal(w.actions.editError.value, "Port 80 clashes with another service");
    assert.equal(w.actions.saveStatus.value.text, "");
  });

  it("publishing a new port keeps the refusal in the shared status, not in editError (unchanged)", async () => {
    const w = setup({ fail: new ApiError(500, "boom", "disk is full") });
    assert.equal(await w.actions.publish(81, ""), false);
    assert.equal(w.actions.saveStatus.value.cls, "field-error-text");
    assert.match(w.actions.saveStatus.value.text, /^Couldn't save: /);
    assert.equal(w.actions.editError.value, "");
  });
});
