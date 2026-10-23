/* The "Home" layout by snapshot (`home-view.ts`): the setup mode, the "how sites open
 * right now" panel, the place of the join card, the card order. Without Vue and DOM. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import {
  activeTabOf, cardOrder, configFieldAccess, HOME_ORDER, homeMode, joinPlace, nodeNameFieldValue, ORDERS, shownStep, statePanel, wizardStep,
} from "./home-view.ts";

describe("homeMode and wizardStep", () => {
  describe("no network", () => {
    it("setup, step 1 — with any mark", () => {
      for (const mark of [null, "browser", "exit", "done"]) {
        assert.equal(homeMode(snap({ network: null }), mark), "wizard", "mark " + mark);
        assert.equal(wizardStep(snap({ network: null }), mark), 1, "mark " + mark);
      }
    });
  });

  describe("there is a network", () => {
    it("mark \"browser\" — setup, step 2", () => {
      assert.equal(homeMode(snap(), "browser"), "wizard");
      assert.equal(wizardStep(snap(), "browser"), 2);
    });

    it("mark \"exit\" — setup, step 3", () => {
      assert.equal(homeMode(snap(), "exit"), "wizard");
      assert.equal(wizardStep(snap(), "exit"), 3);
    });

    it("mark \"done\" — the normal view", () => assert.equal(homeMode(snap(), "done"), "normal"));

    it("no mark — the normal view (a node that was in a network before this stage doesn't see the setup)", () => {
      assert.equal(homeMode(snap(), null), "normal");
    });
  });
});

describe("statePanel", () => {
  describe("join progress by invite", () => {
    const searching = { phase: "searching", main: "Looking for the network…", sub: "waiting" };

    it("searching and entering — a grey panel with a spinner, ahead of the node and route", () => {
      const p = statePanel({ node: { status: "online" }, network: { label: "x" } }, null, searching);
      assert.deepEqual(p, { level: "grey", main: "Looking for the network…", line: "Looking for the network…", sub: "waiting", actions: [], busy: true });
      assert.equal(statePanel(null, "silent", { ...searching, phase: "entering" }).busy, true);
    });

    it("refused — a red panel without a spinner", () => {
      const p = statePanel(null, null, { phase: "refused", main: "Refused.", sub: "" });
      assert.equal(p.level, "red");
      assert.equal(p.main, "Refused.");
      assert.equal(p.busy, false);
    });
  });

  describe("the node is not responding (state === null)", () => {
    it("grey, a hint about the tray icon", () => {
      const p = statePanel(null);
      assert.equal(p.level, "grey");
      assert.equal(p.main, "Mesh Town is not responding");
      assert.ok(p.sub.includes("\"Start " + NET_NAME + "\""));
    });
  });

  describe("the node is starting or stopping", () => {
    it("starting — grey \"Mesh Town is starting…\"", () => {
      const p = statePanel(snap({ node: { status: "starting" } }));
      assert.equal(p.level, "grey");
      assert.equal(p.main, NET_NAME + " is starting…");
    });

    it("stopping — grey \"Mesh Town is stopping…\", without a hint", () => {
      const p = statePanel(snap({ node: { status: "stopping" } }));
      assert.equal(p.level, "grey");
      assert.equal(p.main, NET_NAME + " is stopping…");
      assert.ok(!p.sub, "\"is stopping\" has no hint");
    });
  });

  describe("sites open as usual", () => {
    it("no route — green \"as usual, without Mesh Town\", no buttons", () => {
      const p = statePanel(snap({ route: undefined, peers: [SELF] }));
      assert.equal(p.level, "green");
      assert.equal(p.main, "In the browser, sites open directly");
      assert.deepEqual(p.actions, []);
    });

    it("state direct and there is an exit member — offers to choose it", () => {
      const p = statePanel(snap({ route: { state: "direct", exits: [] }, peers: [SELF, peer({ exit: true })] }));
      assert.equal(p.main, "In the browser, sites open directly");
      assert.equal(p.sub, "To open a blocked site, choose a member below.");
    });

    it("state direct and no one allowed opening sites through them — an explanation instead of an offer", () => {
      const p = statePanel(snap({ route: { state: "direct", exits: [] }, peers: [SELF, peer({ exit: false })] }));
      assert.equal(p.sub, "No member offers their connection as an exit yet.");
    });
  });

  describe("an exit is chosen", () => {
    it("green, the member is named by the active exit, without an id; online · via relay server · 30 ms", () => {
      const p = statePanel(exitSnap({ path: "relay", rtt_ms: 30 }));
      assert.equal(p.level, "green");
      assert.equal(p.main, "In the browser, sites open via peer-one");
      assert.equal(p.sub, "online · via relay server · 30 ms");
    });

    it("path direct — \"direct\" in the tail", () => {
      assert.equal(statePanel(exitSnap({ path: "direct", rtt_ms: 12 })).sub, "online · direct · 12 ms");
    });

    it("no active one in peers[] — the caption is empty, not \"online\"", () => {
      const state = exitSnap({});
      state.peers = [SELF];
      assert.equal(statePanel(state).sub, "");
    });

    it("latency null — no \" ms\"", () => {
      const p = statePanel(exitSnap({ path: "direct", rtt_ms: null }));
      assert.equal(p.sub, "online · direct");
      assert.doesNotMatch(p.sub, /ms/);
    });

    it("path unknown — no tail about the path", () => {
      assert.equal(statePanel(exitSnap({ path: null, rtt_ms: 30 })).sub, "online · 30 ms");
    });

    it("the element with active: true is taken, not the first in the list", () => {
      const state = exitSnap({});
      state.route.exits = [
        { id: "cccc3333", id_full: "C", name: "second", active: false },
        { id: "bbbb2222", id_full: "B", name: "peer-one", active: true },
      ];
      assert.equal(statePanel(state).main, "In the browser, sites open via peer-one");
    });

    it("no active one — \"via the chosen member\"", () => {
      const state = exitSnap({});
      state.route.exits = [{ id: "bbbb2222", id_full: "B", name: "peer-one", active: false }];
      assert.equal(statePanel(state).main, "In the browser, sites open via the chosen member");
    });
  });

  describe("the exit is offline", () => {
    it("one exit — red, named by name, both buttons", () => {
      const state = snap({
        route: { state: "unavailable", exits: [{ id: "bbbb2222", id_full: "B", name: "peer-one", active: false }] },
      });
      const p = statePanel(state);
      assert.equal(p.level, "red");
      assert.equal(p.main, "peer-one is offline — sites may not open or may open directly");
      assert.deepEqual(p.actions, ["pick-other", "direct"]);
    });

    it("two exits — a general phrase without names", () => {
      const state = snap({
        route: {
          state: "unavailable",
          exits: [
            { id: "bbbb2222", id_full: "B", name: "peer-one" },
            { id: "cccc3333", id_full: "C", name: "" },
          ],
        },
      });
      const p = statePanel(state);
      assert.equal(p.level, "red");
      assert.equal(p.main, "The chosen members are offline — sites may not open or may open directly");
      assert.deepEqual(p.actions, ["pick-other", "direct"]);
    });
  });

  describe("incomplete snapshot", () => {
    it("{} and { node: {} } — no exception, with text", () => {
      for (const state of [{}, { node: {} }]) {
        const p = statePanel(state);
        assert.equal(typeof p.main, "string");
        assert.ok(["green", "red", "grey"].includes(p.level));
      }
    });
  });

  describe("texts", () => {
    it("there is no \"null\", \"undefined\" or bare \"through the network\" anywhere", () => {
      const states = [
        null, {}, { node: {} },
        snap({ node: { status: "starting" } }),
        snap({ node: { status: "stopping" } }),
        snap({ route: undefined }),
        snap({ route: { state: "direct", exits: [] } }),
        exitSnap({ path: "relay", rtt_ms: 30 }),
        exitSnap({ path: null, rtt_ms: null }),
        snap({ route: { state: "unavailable", exits: [{ id: "bbbb2222", id_full: "B", name: null }] } }),
        snap({ network: { label: null, id_hex: "n" }, route: { state: "direct", exits: [] } }),
      ];
      for (const state of states) {
        const p = statePanel(state);
        for (const text of [p.main, p.sub]) {
          if (!text) continue;
          assert.doesNotMatch(text, /null|undefined/, text);
          assert.doesNotMatch(text, /through the network(?! Mesh Town)/, text);
        }
      }
    });
  });
});

describe("joinPlace", () => {
  it("no network — the join card on \"Home\"", () => {
    assert.equal(joinPlace(snap({ network: null })), "home");
  });

  it("there is a network — still on \"Home\" (Settings offers the add-network card)", () => assert.equal(joinPlace(snap()), "home"));

  it("there is a network, one member (just created) — still \"Home\"", () => {
    assert.equal(joinPlace(snap({ peers: [SELF] })), "home");
  });
});

describe("cardOrder", () => {
  describe("variant \"V\"", () => {
    it("no exits — the invite goes first", () => {
      const state = snap({ route: { state: "direct", exits: [] }, peers: [SELF, peer({ exit: false })] });
      assert.deepEqual(cardOrder(state, "V"), ["invite", "exit", "row"]);
    });

    it("another member with exit: true — the exit choice goes first", () => {
      const state = snap({ route: { state: "direct", exits: [] }, peers: [SELF, peer({ exit: true })] });
      assert.deepEqual(cardOrder(state, "V"), ["exit", "row"]);
    });

    it("own exit: true doesn't count as an exit", () => {
      const state = snap({ route: { state: "direct", exits: [] }, peers: [{ ...SELF, exit: true }] });
      assert.deepEqual(cardOrder(state, "V"), ["invite", "exit", "row"]);
    });

    it("route.exits is non-empty — the exit choice goes first", () => {
      assert.deepEqual(cardOrder(exitSnap({}), "V"), ["exit", "row"]);
    });
  });

  describe("variants \"A\" and \"B\"", () => {
    it("\"A\" — the exit choice goes first, the invite is in the row of small ones", () => {
      assert.deepEqual(cardOrder(snap(), "A"), ["exit", "row"]);
    });

    it("\"B\" — the invite goes first", () => {
      assert.deepEqual(cardOrder(snap(), "B"), ["invite", "exit", "row"]);
    });
  });
});

const SELF = { self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "me" };

function peer(over = {}) {
  return { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", online: true, exit: false, ...over };
}

// A node snapshot with a network and two members; `over` overrides top-level fields.
function snap(over = {}) {
  return {
    node: { status: "online", id: "aaaa1111" },
    network: { label: "home", id_hex: "net-a" },
    peers: [SELF, peer()],
    route: { state: "direct", exits: [] },
    ...over,
  };
}

// Member B is chosen; `over` — the fields of its row in `peers[]` (path, rtt_ms).
function exitSnap(over) {
  return snap({
    route: { state: "exit", exits: [{ id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", active: true }] },
    peers: [SELF, peer({ exit: true, ...over })],
  });
}

describe("HOME_ORDER", () => {
  it("a known layout variant (a typo in the constant doesn't stay silent)", () => {
    assert.ok(ORDERS.has(HOME_ORDER), "HOME_ORDER = " + HOME_ORDER);
  });
});

describe("nodeNameFieldValue", () => {
  const state = { peers: [{ self: true, name: "from-row" }] };

  it("the name from settings beats everything, including an empty one", () => {
    assert.equal(nodeNameFieldValue({ node: { name: "cfg" } }, state), "cfg");
    assert.equal(nodeNameFieldValue({ node: { name: "" } }, state), "");
  });

  it("not in settings — the self row from the snapshot, then empty", () => {
    assert.equal(nodeNameFieldValue({}, state), "from-row");
    assert.equal(nodeNameFieldValue(null, { peers: [{ self: true, name: "from-row" }] }), "from-row");
    assert.equal(nodeNameFieldValue(undefined, null), "");
  });
});

describe("configFieldAccess", () => {
  it("read_only and cli_overrides from configMeta; without meta — a free field", () => {
    const meta = { read_only: ["node.name"], cli_overrides: ["node.name"] };
    assert.deepEqual(configFieldAccess(meta, "node.name"), { readOnly: true, cliNote: "set at node startup — can't be changed here" });
    assert.deepEqual(configFieldAccess(meta, "exit.enabled"), { readOnly: false, cliNote: "" });
    assert.deepEqual(configFieldAccess(null, "node.name"), { readOnly: false, cliNote: "" });
  });
});

describe("activeTabOf and shownStep", () => {
  it("without an explicit choice the tab follows the mode", () => {
    assert.equal(activeTabOf(null, "wizard"), "wizard");
    assert.equal(activeTabOf(null, "normal"), "main");
  });

  it("an explicit choice wins over the mode", () => {
    assert.equal(activeTabOf("main", "wizard"), "main");
    assert.equal(activeTabOf("wizard", "normal"), "wizard");
  });

  it("a completed setup is shown from step 2, the other steps as they are", () => {
    assert.equal(shownStep(0), 2);
    assert.equal(shownStep(1), 1);
    assert.equal(shownStep(3), 3);
  });
});
