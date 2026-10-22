/* The "Home" actions without Vue (`home-actions.ts`): the exit choice, "Check a site", the node name,
 * the "Allow others" switch, copying the proxy address, the setup mark. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import {
  chooseDirect, chooseExit, copyValue, createHomeForm, findExit, readSetupMark, saveExitServer, saveName,
  writeSetupMark,
} from "./home-actions.ts";

// `saveConfigFields` sends "meshtown:changed" to the page's `document`; node has none — we stub it.
// relies on file isolation; isolate:false in the node project would break it
Object.defineProperty(globalThis, "document", { value: new EventTarget(), configurable: true });

const REMEMBERED = "meshtown.exitsRemembered";
const rejected = (code = "invalid", fields = []) => new ApiError(422, code, "node refused", { fields });
const exitRejected = () => rejected("invalid", [{ path: "route.exits", message: "Exit unavailable" }]);

describe("chooseExit", () => {
  describe("the node accepted", () => {
    it("PUT with route.exits: [id], the choice is remembered, \"Saved\"", async () => {
      const w = setup({ exits: [] });
      w.form.exitChoice = "P";
      assert.equal(await chooseExit(w.form, w.store, "P", w.deps), "saved");
      assert.deepEqual(w.puts[0].route.exits, ["P"]);
      assert.deepEqual(w.remembered(), ["P"]);
      assert.equal(w.form.exitStatus.text, "Saved");
      assert.equal(w.form.exitChoice, "P");
      assert.equal(w.form.exitSavesInFlight, 0);
    });
  });

  describe("the node refused (422)", () => {
    it("the choice went back to the previous one, the status is the field's text, nothing is remembered", async () => {
      const w = setup({ exits: ["Q"], fail: exitRejected() });
      w.form.exitChoice = "P";
      assert.equal(await chooseExit(w.form, w.store, "P", w.deps), "failed");
      assert.equal(w.form.exitChoice, "Q", "the radio button went back to the previous choice");
      assert.equal(w.form.exitStatus.text, "Exit unavailable");
      assert.equal(w.remembered(), null);
      assert.equal(w.form.exitSavesInFlight, 0);
    });

    it("there was no previous choice — \"as usual\" (an empty string)", async () => {
      const w = setup({ exits: [], fail: exitRejected() });
      w.form.exitChoice = "P";
      await chooseExit(w.form, w.store, "P", w.deps);
      assert.equal(w.form.exitChoice, "");
    });
  });
});

describe("chooseExit, two quick clicks", () => {
  describe("the first PUT finished before the second", () => {
    it("no flicker: the choice stays on the second while its PUT is in flight", async () => {
      // given: two PUTs with manual completion
      const w = setupGated();
      const first = chooseExit(w.form, w.store, "A", w.deps);
      w.form.exitChoice = "B"; // this is what the radio button's v-model does on the second click
      const second = chooseExit(w.form, w.store, "B", w.deps);
      // when: A completed
      w.gates[0].resolve();
      await first;
      // then
      assert.equal(w.form.exitChoice, "B", "the radio button doesn't jump to A");
      assert.equal(w.form.exitSavesInFlight, 1);
      // when: B completed
      w.gates[1].resolve();
      await second;
      // then
      assert.equal(w.form.exitChoice, "B");
      assert.equal(w.form.exitSavesInFlight, 0);
    });
  });
});

describe("chooseDirect", () => {
  it("the previous choice is remembered, PUT route.exits: [], \"Saved\"", async () => {
    const w = setup({ exits: ["Q"] });
    w.form.exitChoice = "";
    assert.equal(await chooseDirect(w.form, w.store, w.deps), "saved");
    assert.deepEqual(w.puts[0].route.exits, []);
    assert.deepEqual(w.remembered(), ["Q"]);
    assert.equal(w.form.exitStatus.text, "Saved");
    assert.equal(w.form.exitChoice, "");
  });

  it("the node refused — the choice went back to the previous node", async () => {
    const w = setup({ exits: ["Q"], fail: exitRejected() });
    w.form.exitChoice = "";
    assert.equal(await chooseDirect(w.form, w.store, w.deps), "failed");
    assert.equal(w.form.exitChoice, "Q");
  });
});

describe("findExit", () => {
  it("the field is empty — \"empty\", no requests", async () => {
    const w = setup({});
    w.form.findDraft = "   ";
    assert.equal(await findExit(w.form, w.deps), "empty");
    assert.equal(w.calls.length, 0);
    assert.equal(w.form.findDomain, "");
    assert.equal(w.form.findError, "Enter a site address, for example example.com");
  });

  it("network failure — \"failed\", \"Couldn't check the site: …\", the wait is cleared", async () => {
    const w = setup({ fail: new ApiError(500, "x", "The node is not responding") });
    w.form.findDraft = "example.com";
    assert.equal(await findExit(w.form, w.deps), "failed");
    assert.equal(w.form.findError, "Couldn't check the site: the node is not responding");
    assert.equal(w.form.findDomain, "");
  });

  it("the domain went into the POST, the field is cleared, the wait goes by the domain", async () => {
    const w = setup({});
    w.form.findDraft = " example.com ";
    assert.equal(await findExit(w.form, w.deps), "sent");
    assert.deepEqual(w.calls, [["POST", "/api/actions/find-exit", { domain: "example.com" }]]);
    assert.equal(w.form.findDraft, "");
    assert.equal(w.form.findDomain, "example.com");
  });

  it("409 \"busy\" — the domain stays, the wait is not cancelled", async () => {
    const w = setup({ fail: new ApiError(409, "busy", "busy") });
    w.form.findDraft = "example.com";
    assert.equal(await findExit(w.form, w.deps), "sent");
    assert.equal(w.form.findDomain, "example.com");
    assert.equal(w.form.findError, "");
  });
});

describe("saveName", () => {
  describe("the name didn't change", () => {
    it("\"same\", no requests", async () => {
      const w = setup({});
      w.form.name.original = "old";
      w.form.name.draft = "old";
      assert.equal(await saveName(w.form, w.store, w.deps), "same");
      assert.equal(w.calls.length, 0);
    });
  });

  describe("the name changed", () => {
    it("PUT node.name, \"Saved\", the saved value is updated", async () => {
      const w = setup({});
      w.form.name.original = "old";
      w.form.name.draft = " new ";
      assert.equal(await saveName(w.form, w.store, w.deps), "saved");
      assert.equal(w.puts[0].node.name, "new");
      assert.equal(w.form.name.original, "new");
      assert.equal(w.form.name.status.text, "Saved");
    });

    it("cleared — an empty name is saved", async () => {
      const w = setup({});
      w.form.name.original = "old";
      w.form.name.draft = "";
      assert.equal(await saveName(w.form, w.store, w.deps), "saved");
      assert.equal(w.puts[0].node.name, "");
    });

    it("the node refused — \"failed\", the error text in the status, the saved one stays as before (a repeated Enter works)", async () => {
      const w = setup({ fail: rejected("invalid", [{ path: "node.name", message: "Name is taken" }]) });
      w.form.name.original = "old";
      w.form.name.draft = "new";
      assert.equal(await saveName(w.form, w.store, w.deps), "failed");
      assert.equal(w.form.name.original, "old");
      assert.equal(w.form.name.status.text, "Name is taken");
    });
  });
});

describe("saveExitServer", () => {
  it("PUT exit.enabled, \"Saved\"", async () => {
    const w = setup({});
    assert.equal(await saveExitServer(w.form, w.store, true, w.deps), "saved");
    assert.equal(w.puts[0].exit.enabled, true);
    assert.equal(w.form.exitServerStatus.text, "Saved");
  });

  it("the node refused — \"failed\", the error text", async () => {
    const w = setup({ fail: rejected("invalid", [{ path: "exit.enabled", message: "Not allowed" }]) });
    assert.equal(await saveExitServer(w.form, w.store, false, w.deps), "failed");
    assert.equal(w.form.exitServerStatus.text, "Not allowed");
  });
});

describe("copyValue", () => {
  it("no clipboard — \"fallback\", \"Copied\" is not shown", async () => {
    const w = setup({ clipboard: null });
    assert.equal(await copyValue(w.form, "host", "127.0.0.1", w.deps), "fallback");
    assert.equal(w.form.browser.copied, "");
    assert.equal(w.form.browser.note, "Clipboard unavailable — select the address in the step and copy it manually.");
  });

  it("there is a clipboard — \"copied\", the value is written, \"Copied\" goes out after 2000 ms", async () => {
    const w = setup({});
    assert.equal(await copyValue(w.form, "port", "1080", w.deps), "copied");
    assert.deepEqual(w.written, ["1080"]);
    assert.equal(w.form.browser.copied, "port");
    assert.equal(w.timers[0].ms, 2000);
    w.timers[0].fn();
    assert.equal(w.form.browser.copied, "");
  });
});

describe("setup mark", () => {
  it("what was written is read back", () => {
    const w = setup({});
    writeSetupMark("exit", w.storage);
    assert.equal(readSetupMark(w.storage), "exit");
  });

  it("nothing was written — null", () => assert.equal(readSetupMark(setup({}).storage), null));

  it("the storage throws — reading gives null, writing is silent", () => {
    const broken = setup({ brokenStorage: true }).storage;
    assert.equal(readSetupMark(broken), null);
    assert.doesNotThrow(() => writeSetupMark("browser", broken));
  });
});

function fakeStorage(broken) {
  const data = new Map();
  const fail = () => { throw new Error("localStorage unavailable"); };
  return {
    data,
    getItem: broken ? fail : (k) => (data.has(k) ? data.get(k) : null),
    setItem: broken ? fail : (k, v) => { data.set(k, String(v)); },
  };
}

// Preconditions: the chosen exits in the config, the node's refusal, the clipboard (writing by default), the storage.
function setup({ exits = [], fail = null, clipboard, brokenStorage = false }) {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "n" }, peers: [] };
  store.config = { route: { exits: [...exits] }, node: { name: "old" }, exit: { enabled: false } };
  store.configRev = 1;
  const calls = [];
  const puts = [];
  const written = [];
  const timers = [];
  const storage = fakeStorage(brokenStorage);
  const api = async (method, path, body) => {
    calls.push([method, path, structuredClone(body)]);
    if (fail) throw fail;
    if (method === "PUT") puts.push(structuredClone(body.config));
    return method === "PUT" ? { config: body.config, rev: 2 } : {};
  };
  const buffer = clipboard === undefined ? { writeText: async (t) => { written.push(t); } } : clipboard;
  const deps = {
    api, storage, clipboard: buffer,
    setTimer: (fn, ms) => { timers.push({ fn, ms }); return timers.length; },
    clearTimer: () => {},
  };
  const remembered = () => (storage.data.has(REMEMBERED) ? JSON.parse(storage.data.get(REMEMBERED)) : null);
  return { form: createHomeForm(), store, calls, puts, written, timers, storage, deps, remembered };
}

// Each PUT hangs until `gates[i].resolve()` is called; there are no GETs.
function setupGated() {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "n" }, peers: [] };
  store.config = { route: { exits: [] }, node: { name: "old" }, exit: { enabled: false } };
  store.configRev = 1;
  const gates = [];
  const api = async (method, path, body) => {
    const gate = { resolve: null };
    const held = new Promise((resolve) => { gate.resolve = resolve; });
    gates.push(gate);
    await held;
    return { config: structuredClone(body.config), rev: 2 };
  };
  const storage = fakeStorage(false);
  const deps = { api, storage, setTimer: () => 1, clearTimer: () => {} };
  return { form: createHomeForm(), store, gates, deps };
}
