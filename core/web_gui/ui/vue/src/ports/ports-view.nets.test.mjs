/* The "Ports" section across several networks: taken local numbers, published ports of every network, per-network audiences. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { defaultLocalPort, netRowKey, openedAfterOpen } from "./ports-view.ts";
import * as view from "./ports-view.ts";
import { createStore } from "../core/store.ts";

describe("defaultLocalPort: numbers taken in other networks", () => {
  describe("the candidate (port + 2000) is free", () => {
    it("is the candidate, as before", () => {
      assert.equal(defaultLocalPort(80, new Set([5000])), 2080);
      assert.equal(defaultLocalPort(80), 2080);
    });
  });

  describe("the candidate is taken", () => {
    it("is the next free number up", () => {
      assert.equal(defaultLocalPort(9000, new Set([11000])), 11001);
    });

    it("skips every taken number in a row", () => {
      assert.equal(defaultLocalPort(80, new Set([2080, 2081])), 2082);
    });

    it("a taken number above the free candidate does not matter", () => {
      assert.equal(defaultLocalPort(9000, new Set([11001])), 11000);
    });
  });

  describe("the candidate does not fit in 65535", () => {
    it("is the member's own port when it is free", () => {
      assert.equal(defaultLocalPort(65000, new Set([65535])), 65000);
    });
  });

  describe("everything from the candidate up to 65535 is taken", () => {
    it("continues from 1024", () => {
      const taken = new Set();
      for (let n = 65000; n <= 65535; n++) taken.add(n);
      assert.equal(defaultLocalPort(63000, taken), 1024);
    });

    it("skips the taken numbers from 1024 too", () => {
      const taken = new Set([1024, 1025]);
      for (let n = 65000; n <= 65535; n++) taken.add(n);
      assert.equal(defaultLocalPort(63000, taken), 1026);
    });
  });

  describe("every number is taken", () => {
    it("returns the original candidate (the node will refuse it)", () => {
      const taken = new Set();
      for (let n = 1; n <= 65535; n++) taken.add(n);
      assert.equal(defaultLocalPort(9000, taken), 11000);
    });
  });
});

describe("openedAfterOpen: taken numbers", () => {
  describe("no local number given, the candidate is taken", () => {
    it("the entry gets the next free number", () => {
      const list = openedAfterOpen([], "NODE-B", 9000, undefined, new Set([11000]));
      assert.deepEqual(list, [{ node: "NODE-B", port: 9000, local: 11001 }]);
    });
  });

  describe("a local number is given", () => {
    it("it is written as is, taken or not", () => {
      const list = openedAfterOpen([], "NODE-B", 9000, 11000, new Set([11000]));
      assert.deepEqual(list, [{ node: "NODE-B", port: 9000, local: 11000 }]);
    });
  });
});

describe("localUsedText", () => {
  describe("one network", () => {
    it("names only the member", () => {
      assert.equal(view.localUsedText(2080, { peerLabel: "peer-one", net: null }), "2080 is already used by peer-one");
    });
  });

  describe("two networks", () => {
    it("names the member and the network", () => {
      assert.equal(view.localUsedText(2080, { peerLabel: "peer-one", net: "friends" }), "2080 is already used by peer-one (friends)");
    });
  });
});

describe("takenLocals", () => {
  describe("opened ports in two networks", () => {
    it("collects the local numbers of every visible network, with the holder", () => {
      const store = twoNetStore({ a: [{ node: "NODE-A", port: 80, local: 2080 }], b: [{ node: "NODE-B", port: 9000, local: 11000 }] });
      const taken = view.takenLocals(store);
      assert.deepEqual([...taken.keys()].sort((x, y) => x - y), [2080, 11000]);
      assert.deepEqual(taken.get(2080), { slot: "main", node: "NODE-A", port: 80 });
      assert.deepEqual(taken.get(11000), { slot: "k7q2xa", node: "NODE-B", port: 9000 });
    });

    it("leaves out the pair being edited", () => {
      const store = twoNetStore({ a: [{ node: "NODE-A", port: 80, local: 2080 }], b: [{ node: "NODE-B", port: 9000, local: 11000 }] });
      const taken = view.takenLocals(store, { slot: "k7q2xa", node: "NODE-B", port: 9000 });
      assert.deepEqual([...taken.keys()], [2080]);
    });

    it("the same node and port in another network is not the edited pair", () => {
      const store = twoNetStore({ a: [{ node: "NODE-B", port: 9000, local: 2080 }], b: [{ node: "NODE-B", port: 9000, local: 11000 }] });
      const taken = view.takenLocals(store, { slot: "k7q2xa", node: "NODE-B", port: 9000 });
      assert.deepEqual([...taken.keys()], [2080]);
    });
  });

  describe("no opened ports", () => {
    it("is empty", () => {
      assert.equal(view.takenLocals(twoNetStore({ a: [], b: [] })).size, 0);
    });
  });
});

describe("my published ports across networks", () => {
  describe("port 8080 is published in both networks", () => {
    it("is one row, shared in both, the main network first", () => {
      const rows = view.mergeOwn(publishStore({ a: [pub(8080, "web", "all")], b: [pub(8080, "site", ["ID-PEER-B"])] }));
      assert.equal(rows.length, 1);
      assert.equal(rows[0].port, 8080);
      assert.equal(rows[0].name, "web", "the name of the first network");
      assert.deepEqual(rows[0].shared.map((s) => s.slot), ["main", "k7q2xa"]);
      assert.deepEqual(rows[0].shared.map((s) => s.audience), ["all", ["ID-PEER-B"]]);
    });

    // The "all" part of the cell uses PORTS_AUDIENCE_ALL ("all members").
    it("the cell groups the audience by network", () => {
      const store = publishStore({ a: [pub(8080, "web", "all")], b: [pub(8080, "web", ["ID-PEER-B"])] });
      const row = view.mergeOwn(store)[0];
      assert.equal(view.mergedAudienceText(store, row), "friends: all · my devices: alice");
    });
  });

  describe("port 7070 is published in the second network only", () => {
    it("is one row shared in that network only", () => {
      const rows = view.mergeOwn(publishStore({ a: [], b: [pub(7070, "db", "all")] }));
      assert.equal(rows.length, 1);
      assert.deepEqual(rows[0].shared.map((s) => s.slot), ["k7q2xa"]);
    });
  });

  describe("different ports in different networks", () => {
    it("are separate rows", () => {
      const rows = view.mergeOwn(publishStore({ a: [pub(80, "a", "all")], b: [pub(81, "b", "all")] }));
      assert.deepEqual(rows.map((r) => r.port), [80, 81]);
    });
  });

  describe("the status", () => {
    it("is the one of the first network that has the row", () => {
      const store = publishStore({ a: [], b: [pub(7070, "db", "all")] });
      store.slots.k7q2xa.state.ports.published = [{ port: 7070, name: "db", local: "ok" }];
      assert.equal(view.mergeOwn(store)[0].local, "ok");
    });
  });
});

describe("per-network audience of the form", () => {
  describe("netGroups", () => {
    it("is one group per network that holds a key, with that network's members as candidates", () => {
      const groups = view.netGroups(publishStore({ a: [], b: [] }));
      assert.deepEqual(groups.map((g) => g.slot), ["main", "k7q2xa"]);
      assert.deepEqual(groups.map((g) => g.label), ["friends", "my devices"]);
      assert.deepEqual(groups[0].candidates, []);
      assert.deepEqual(groups[1].candidates.map((c) => c.idFull), ["ID-PEER-B"]);
    });
  });

  describe("audiencesOf", () => {
    const groups = view.netGroups(publishStore({ a: [], b: [] }));

    it("a network with nothing checked gets null: the port is not published there", () => {
      const res = view.audiencesOf(groups, { main: { all: true, ids: [] }, k7q2xa: { all: false, ids: [] } });
      assert.deepEqual(res, { main: "all", k7q2xa: null });
    });

    it("a network with checked members gets their ids, a network without a choice gets null", () => {
      const res = view.audiencesOf(groups, { k7q2xa: { all: false, ids: ["ID-PEER-B"] } });
      assert.deepEqual(res, { main: null, k7q2xa: ["ID-PEER-B"] });
    });

    it("checked ids that are not among the network's members are dropped", () => {
      const res = view.audiencesOf(groups, { k7q2xa: { all: false, ids: ["ID-GONE"] } });
      assert.equal(res.k7q2xa, null);
    });
  });

  describe("selectionOf", () => {
    it("maps the audience of an existing entry to a choice, and a missing entry to nothing checked", () => {
      assert.deepEqual(view.selectionOf("all"), { all: true, ids: [] });
      assert.deepEqual(view.selectionOf(["x"]), { all: false, ids: ["x"] });
      assert.deepEqual(view.selectionOf(undefined), { all: false, ids: [] });
    });
  });
});

describe("ports shared with you across networks", () => {
  describe("two networks", () => {
    it("rows stay per network and carry the network name; the same member in both is two rows", () => {
      const store = publishStore({ a: [], b: [] });
      const row = { node: "NODE-X", node_name: "bob", online: true, port: 80, name: "", published: true, opened: null };
      store.slots.main.state.ports.network = [row];
      store.slots.k7q2xa.state.ports.network = [row];
      const items = view.netPortItems(store);
      assert.deepEqual(items.map((i) => [i.slot, i.net]), [["main", "friends"], ["k7q2xa", "my devices"]]);
    });

    it("the member label is \"<peer> (<net>)\"", () => {
      assert.equal(view.peerNetLabel("bob", "my devices"), "bob (my devices)");
    });
  });

  describe("the row key", () => {
    it("includes the slot, so the same node and port in two networks do not collide", () => {
      assert.notEqual(netRowKey({ node: "ab", port: 80 }, "main"), netRowKey({ node: "ab", port: 80 }, "k7q2xa"));
      assert.equal(netRowKey({ node: "ab", port: 80 }, "k7q2xa"), "k7q2xa/ab/80");
    });
  });
});

// ---------------------------------------------------------------------------------------------------------------------
// Helpers: stores with two networks, A = "main" "friends", B = "k7q2xa" "my devices".

function brief(id, label, idHex) {
  return {
    id, label, id_hex: idHex, status: "running", exit_enabled: false, proxy: id === "main",
    route_state: "direct", exits: [], joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0,
  };
}

function slice(b, config, peers) {
  const state = { peers, ports: { published: [], network: [] } };
  return { id: b.id, brief: b, state, members: null, config, configRev: 1, configMeta: null, bootId: null };
}

function twoNetStore({ a, b, publishedA = [], publishedB = [], peersA = [], peersB = [] }) {
  const store = createStore();
  const nets = [brief("main", "friends", "net-aaaa"), brief("k7q2xa", "my devices", "net-bbbb")];
  store.nets = nets;
  store.config = { ports: { published: publishedA, opened: a } };
  store.slots.main = slice(nets[0], store.config, peersA);
  store.slots.k7q2xa = slice(nets[1], { ports: { published: publishedB, opened: b } }, peersB);
  return store;
}

function publishStore({ a, b }) {
  const peerB = { id: "alice001", id_full: "ID-PEER-B", name: "alice", self: false, online: true };
  return twoNetStore({ a: [], b: [], publishedA: a, publishedB: b, peersB: [peerB] });
}

function pub(port, name, audience) {
  return { port, name, for: audience };
}
