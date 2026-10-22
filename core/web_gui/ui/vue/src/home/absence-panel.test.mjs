/* The "Home" panel when the node is silent (`home-view.ts::statePanel(state, absent)`): the second argument is the reason
 * why the page gets no snapshots (`shell/conn.ts::absence`). While it is set, the panel doesn't show a
 * stale snapshot and says the same as the header status line. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as words from "../common/words.ts";
import * as connModule from "../shell/conn.ts";
import { statusLine } from "../shell/status-line.ts";
import { statePanel } from "./home-view.ts";

const HINT = "Start it: the Mesh Town icon near the clock → \"Start Mesh Town\".";
const NOT_RESPONDING = "Mesh Town is not responding";
const STOPPED = "Mesh Town is stopped";
const NEED_LINK = "A new page link is needed";

describe("statePanel with a reason for the node's silence", () => {
  describe("the node is silent (silent)", () => {
    it("the snapshot is \"stopping\" — grey \"Mesh Town is not responding\" with a hint about the icon, no buttons", () => {
      const panel = statePanel(stoppingSnap(), "silent");
      assert.deepEqual(panel, { level: "grey", main: NOT_RESPONDING, line: NOT_RESPONDING, sub: HINT, actions: [] });
    });

    it("the snapshot is \"exit chosen\" — not \"sites go via …\" but \"not responding\"", () => {
      const panel = statePanel(exitSnap(), "silent");
      assert.equal(panel.level, "grey");
      assert.equal(panel.main, NOT_RESPONDING);
    });

    it("the snapshot is \"exit offline\" — no red buttons \"Choose another\" and \"Open as usual\"", () => {
      const panel = statePanel(unavailableSnap(), "silent");
      assert.deepEqual(panel.actions, []);
      assert.equal(panel.level, "grey");
    });

    it("the snapshot is \"as usual\" — also \"not responding\": a silent node has no green panel", () => {
      const panel = statePanel(snap(), "silent");
      assert.equal(panel.level, "grey");
      assert.equal(panel.main, NOT_RESPONDING);
    });
  });

  describe("the node is stopped (stopped)", () => {
    it("grey \"Mesh Town is stopped\" with a hint about the icon, no buttons", () => {
      const panel = statePanel(stoppingSnap(), "stopped");
      assert.deepEqual(panel, { level: "grey", main: STOPPED, line: STOPPED, sub: HINT, actions: [] });
    });

    it("the snapshot is \"as usual\" — not a green panel but \"stopped\"", () => {
      const panel = statePanel(snap(), "stopped");
      assert.equal(panel.level, "grey");
      assert.equal(panel.main, STOPPED);
    });
  });

  describe("a new link is needed (token)", () => {
    it("grey \"A new page link is needed\", no hint and no buttons", () => {
      const panel = statePanel(snap(), "token");
      assert.deepEqual(panel, { level: "grey", main: NEED_LINK, line: NEED_LINK, sub: "", actions: [] });
    });
  });

  describe("no reason (null or the argument not passed)", () => {
    for (const absent of [undefined, null]) {
      const named = absent === null ? "null" : "not passed";

      it(`${named}: "stopping" — grey "Mesh Town is stopping…" without a hint`, () => {
        const panel = statePanel(stoppingSnap(), absent);
        assert.deepEqual(panel, { level: "grey", main: "Mesh Town is stopping…", line: "Mesh Town is stopping…", sub: "", actions: [] });
      });

      it(`${named}: exit chosen — green "via peer-one", no id`, () => {
        const panel = statePanel(exitSnap(), absent);
        assert.equal(panel.level, "green");
        assert.equal(panel.main, "In the browser, sites open via peer-one");
      });

      it(`${named}: "as usual" — green, no buttons`, () => {
        const panel = statePanel(snap(), absent);
        assert.equal(panel.level, "green");
        assert.equal(panel.main, "In the browser, sites open directly");
        assert.deepEqual(panel.actions, []);
      });

      it(`${named}: exit offline — red with the buttons "pick-other" and "direct"`, () => {
        const panel = statePanel(unavailableSnap(), absent);
        assert.equal(panel.level, "red");
        assert.deepEqual(panel.actions, ["pick-other", "direct"]);
      });

      it(`${named}: no snapshot — grey "Mesh Town is not responding" with a hint, as before`, () => {
        const panel = statePanel(null, absent);
        assert.deepEqual(panel, { level: "grey", main: NOT_RESPONDING, line: NOT_RESPONDING, sub: HINT, actions: [] });
      });
    }
  });

  describe("no snapshot at all (state === null)", () => {
    it("with the reason \"silent\" — the same panel as without a reason, verbatim", () => {
      assert.deepEqual(statePanel(null, "silent"), statePanel(null));
    });
  });
});

describe("ABSENCE_TEXT and ABSENCE_HINT (common/words.ts)", () => {
  it("ABSENCE_TEXT.token — \"A new page link is needed\"", () => {
    assert.equal(absenceText().token, NEED_LINK);
  });

  it("ABSENCE_TEXT.silent — the same as NOT_RESPONDING_TEXT", () => {
    assert.equal(absenceText().silent, words.NOT_RESPONDING_TEXT);
    assert.equal(absenceText().silent, NOT_RESPONDING);
  });

  it("ABSENCE_TEXT.stopped — \"Mesh Town is stopped\"", () => {
    assert.equal(absenceText().stopped, STOPPED);
  });

  it("ABSENCE_HINT.token — empty: the new link is visible on the strip", () => {
    assert.equal(absenceHint().token, "");
  });

  it("ABSENCE_HINT.silent and .stopped — the hint \"Start it: the Mesh Town icon near the clock → …\"", () => {
    assert.equal(absenceHint().silent, HINT);
    assert.equal(absenceHint().stopped, HINT);
  });
});

describe("the \"Home\" panel and the header status line say the same", () => {
  const states = [
    ["\"stopping\"", stoppingSnap],
    ["\"as usual\"", snap],
    ["exit chosen", exitSnap],
  ];
  const conns = [
    ["node is silent", silentConn, NOT_RESPONDING],
    ["node is stopped", stoppedConn, STOPPED],
    ["waiting for a new link", tokenConn, NEED_LINK],
  ];

  for (const [stateName, makeState] of states) {
    for (const [connName, makeConn, text] of conns) {
      it(`snapshot ${stateName}, ${connName} — both say "${text}"`, () => {
        const state = makeState();
        const conn = makeConn();
        assert.equal(statusLine(state, conn).text, text, "header");
        assert.equal(statePanel(state, absenceOf(conn)).main, text, "panel");
      });
    }
  }

  it("no snapshot, the node is silent — both say \"Mesh Town is not responding\"", () => {
    const conn = silentConn();
    assert.equal(statusLine(null, conn).text, NOT_RESPONDING, "header");
    assert.equal(statePanel(null, absenceOf(conn)).main, NOT_RESPONDING, "panel");
  });

  it("no snapshot, waiting for a new link — both say \"A new page link is needed\"", () => {
    const conn = tokenConn();
    assert.equal(statusLine(null, conn).text, NEED_LINK, "header");
    assert.equal(statePanel(null, absenceOf(conn)).main, NEED_LINK, "panel");
  });

  it("the connection is fine — both the panel and the header follow the snapshot: \"stopping…\"", () => {
    const conn = connModule.createConn();
    const state = stoppingSnap();
    assert.equal(absenceOf(conn), null);
    assert.equal(statePanel(state, absenceOf(conn)).main, "Mesh Town is stopping…");
    assert.equal(statusLine(state, conn).text, "Mesh Town is stopping…");
  });
});

const SELF = { self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "me" };
const PEER = { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", online: true, exit: true, path: "direct", rtt_ms: 30 };

// A node snapshot with a network and the "as usual" route; `over` overrides top-level fields.
function snap(over = {}) {
  return {
    node: { status: "online", id: "aaaa1111" },
    network: { label: "home", id_hex: "net-a" },
    peers: [SELF, PEER],
    route: { state: "direct", exits: [] },
    ...over,
  };
}

// The last snapshot before the node went silent: "stopping".
function stoppingSnap() {
  return snap({ node: { status: "stopping", id: "aaaa1111" } });
}

function exitSnap() {
  return snap({ route: { state: "exit", exits: [{ id: PEER.id, id_full: PEER.id_full, name: PEER.name, active: true }] } });
}

function unavailableSnap() {
  return snap({ route: { state: "unavailable", exits: [{ id: PEER.id, id_full: PEER.id_full, name: PEER.name, active: false }] } });
}

// Two poll failures in a row: the "not responding" strip appeared on the connection.
function silentConn() {
  const conn = connModule.createConn();
  connModule.onFailure(conn);
  connModule.onFailure(conn);
  return conn;
}

function stoppedConn() {
  const conn = connModule.createConn();
  connModule.onStopping(conn);
  return conn;
}

function tokenConn() {
  const conn = connModule.createConn();
  connModule.onBadToken(conn);
  return conn;
}

// Through the namespace: no export — the test assertion fails, not the file parse.
function absenceOf(conn) {
  assert.equal(typeof connModule.absence, "function", "shell/conn.ts has no export absence(conn)");
  return connModule.absence(conn);
}

function absenceText() {
  assert.ok(words.ABSENCE_TEXT, "common/words.ts has no export ABSENCE_TEXT");
  return words.ABSENCE_TEXT;
}

function absenceHint() {
  assert.ok(words.ABSENCE_HINT, "common/words.ts has no export ABSENCE_HINT");
  return words.ABSENCE_HINT;
}
