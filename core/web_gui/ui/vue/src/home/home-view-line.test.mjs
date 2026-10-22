/* `statePanel(...).line` — the short route phrase for the header: "Sites open …" in a browser, "Exit: …" in the app.
 * `main` (the Home panel text) stays as it was in a browser; names carry no ids until two members share a name. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { statePanel } from "./home-view.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android";
const BROWSER_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

describe("statePanel line", () => {
  describe("in a browser", () => {
    it("direct — main keeps \"In the browser…\", line is \"Sites open directly\"", () => {
      const { w } = given(BROWSER_UA, snap());
      const p = statePanel(w);
      assert.equal(p.main, "In the browser, sites open directly");
      assert.equal(p.line, "Sites open directly");
    });

    it("an exit — main \"In the browser, sites open via peer-one\", line \"Sites open via peer-one\"", () => {
      const { w } = given(BROWSER_UA, exitSnap());
      const p = statePanel(w);
      assert.equal(p.main, "In the browser, sites open via peer-one");
      assert.equal(p.line, "Sites open via peer-one");
    });

    it("an exit that shares its name with another member — the id tells them apart, in main and in line", () => {
      const twin = peer({ id: "cccc3333", id_full: "C".repeat(64), name: "Peer-One" });
      const state = exitSnap();
      state.peers.push(twin);
      const p = statePanel(given(BROWSER_UA, state).w);
      assert.equal(p.main, "In the browser, sites open via peer-one (bbbb2222)");
      assert.equal(p.line, "Sites open via peer-one (bbbb2222)");
    });

    it("the exit is offline — line equals main, the name without an id", () => {
      const route = { state: "unavailable", exits: [{ id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one" }] };
      const p = statePanel(given(BROWSER_UA, snap({ route })).w);
      assert.equal(p.main, "peer-one is offline — sites may not open or may open directly");
      assert.equal(p.line, p.main);
    });
  });

  describe("in the app", () => {
    it("direct — main and line are \"Exit: none, sites open directly\"", () => {
      const p = statePanel(given(APP_UA, snap()).w);
      assert.equal(p.main, "Exit: none, sites open directly");
      assert.equal(p.line, "Exit: none, sites open directly");
    });

    it("an exit — main and line are \"Exit: peer-one\"", () => {
      const p = statePanel(given(APP_UA, exitSnap()).w);
      assert.equal(p.main, "Exit: peer-one");
      assert.equal(p.line, "Exit: peer-one");
    });

    it("the exit is offline — the text as in a browser, line equals main", () => {
      const route = { state: "unavailable", exits: [{ id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one" }] };
      const p = statePanel(given(APP_UA, snap({ route })).w);
      assert.equal(p.main, "peer-one is offline — sites may not open or may open directly");
      assert.equal(p.line, p.main);
    });
  });

  describe("other branches — line equals main", () => {
    it("no snapshot", () => {
      const p = statePanel(given(BROWSER_UA, null).w);
      assert.equal(p.line, p.main);
    });

    it("the node is stopping", () => {
      const p = statePanel(given(BROWSER_UA, snap({ node: { status: "stopping" } })).w);
      assert.equal(p.line, "Mesh Town is stopping…");
    });

    it("a silent node (absence)", () => {
      const p = statePanel(given(APP_UA, snap()).w, "silent");
      assert.equal(p.line, p.main);
    });
  });
});

/** Sets the user agent for the page-type check and hands the snapshot back. */
function given(userAgent, state) {
  vi.stubGlobal("navigator", { userAgent });
  return { w: state };
}

const SELF = { self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "me" };

function peer(over = {}) {
  return { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", online: true, exit: false, ...over };
}

function snap(over = {}) {
  return {
    node: { status: "online", id: "aaaa1111" },
    network: { label: "home", id_hex: "net-a" },
    peers: [SELF, peer()],
    route: { state: "direct", exits: [] },
    ...over,
  };
}

function exitSnap() {
  return snap({
    route: { state: "exit", exits: [{ id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", active: true }] },
    peers: [SELF, peer({ exit: true })],
  });
}
