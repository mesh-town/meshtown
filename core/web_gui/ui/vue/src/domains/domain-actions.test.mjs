/* Core of the "Site checks" actions (`domain-actions.ts`) with a stub `api`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createDomainActions, emptyUi } from "./domain-actions.ts";
import { ADD_HINT, CHECK_ALL_CONFLICT } from "./domains-view.ts";

// `respond` returns a promise for each call; a hanging request is `hang()`.
function setup(respond = async () => ({})) {
  const ui = emptyUi();
  const calls = [];
  const api = (method, path, body) => {
    calls.push({ method, path, body });
    return respond(method, path, body);
  };
  return { ui, calls, actions: createDomainActions(ui, api) };
}

const hanging = () => {
  let release;
  const p = new Promise((r) => { release = r; });
  return { p, release };
};

describe("recheck / remove", () => {
  it("api refusal clears \"busy\" and sets \"Couldn't recheck …\"", async () => {
    const w = setup(async () => { throw new ApiError(500, "x", "Failure"); });
    await w.actions.recheck("a.org");
    assert.equal(w.ui.busy["a.org"], false);
    assert.equal(w.ui.errors["a.org"], "Couldn't recheck a.org: failure");
  });

  it("the error text is taken from the response body", async () => {
    const w = setup(async () => { throw new ApiError(400, "x", "failure", { message: "not allowed" }); });
    await w.actions.remove("a.org");
    assert.equal(w.ui.errors["a.org"], "Couldn't remove a.org: not allowed");
  });

  it("a double call while a request hangs sends one request", async () => {
    const h = hanging();
    const w = setup(() => h.p);
    const first = w.actions.recheck("a.org");
    await w.actions.recheck("a.org");
    assert.equal(w.calls.length, 1);
    assert.equal(w.ui.busy["a.org"], true);
    h.release();
    await first;
    assert.equal(w.ui.busy["a.org"], false);
  });

  it("a new call clears the row's previous error; remove encodes the name", async () => {
    const w = setup();
    w.ui.errors["a b"] = "old";
    await w.actions.remove("a b");
    assert.equal("a b" in w.ui.errors, false);
    assert.equal(w.calls[0].path, "/api/domains?name=a%20b");
    assert.equal(w.calls[0].method, "DELETE");
  });
});

describe("checkAll", () => {
  it("409 — the conflict text, \"busy\" cleared", async () => {
    const w = setup(async () => { throw new ApiError(409, "busy", "in progress"); });
    await w.actions.checkAll();
    assert.equal(w.ui.checkAllStatus, CHECK_ALL_CONFLICT);
    assert.equal(w.ui.checkAllBusy, false);
  });

  it("other refusal — \"Couldn't start the check: …\"", async () => {
    const w = setup(async () => { throw new ApiError(500, "x", "broke"); });
    await w.actions.checkAll();
    assert.equal(w.ui.checkAllStatus, "Couldn't start the check: broke");
  });

  it("a second call while the request is busy sends nothing", async () => {
    const h = hanging();
    const w = setup(() => h.p);
    const first = w.actions.checkAll();
    await w.actions.checkAll();
    assert.equal(w.calls.length, 1);
    h.release();
    await first;
    assert.equal(w.ui.checkAllBusy, false);
  });
});

describe("add", () => {
  it("empty string or spaces — false, no request", async () => {
    const w = setup();
    assert.equal(await w.actions.add("   "), false);
    assert.equal(w.calls.length, 0);
  });

  it("success — true; the domain trimmed at the edges goes into the request, the previous error is cleared", async () => {
    const w = setup();
    w.ui.addError = "old";
    assert.equal(await w.actions.add("  example.com "), true);
    assert.deepEqual(w.calls[0].body, { domain: "example.com" });
    assert.equal(w.ui.addError, "");
  });

  it("a pasted address goes as is: the node parses the URL down to the domain", async () => {
    const w = setup();
    await w.actions.add("https://example.com/path");
    assert.deepEqual(w.calls[0].body, { domain: "https://example.com/path" });
  });

  it("refusal — false, \"Couldn't add the site: …\" or with a hint", async () => {
    const w = setup(async () => { throw new ApiError(400, "x", "failure", { message: "bad address" }); });
    assert.equal(await w.actions.add("x"), false);
    assert.equal(w.ui.addError, "Couldn't add the site: bad address");
    const v = setup(async () => { throw new Error(""); });
    await v.actions.add("x");
    assert.equal(v.ui.addError, "Couldn't add the site — " + ADD_HINT);
  });
});

describe("onRowsBusy (what the watch calls)", () => {
  it("true→false clears the status; true→true and false→false leave it", () => {
    const w = setup();
    w.ui.checkAllStatus = "in progress";
    w.actions.onRowsBusy(true, true);
    w.actions.onRowsBusy(false, false);
    assert.equal(w.ui.checkAllStatus, "in progress");
    w.actions.onRowsBusy(false, true);
    assert.equal(w.ui.checkAllStatus, "");
  });
});
