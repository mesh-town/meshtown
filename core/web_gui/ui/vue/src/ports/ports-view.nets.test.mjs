/* Shared services across several networks: taken local numbers are counted over all networks, the published and shared
 * rows are those of one network. */

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

  describe("a pair of a network other than the one being edited", () => {
    it("is still taken: 127.0.0.1 is one per computer", () => {
      const store = twoNetStore({ a: [{ node: "NODE-A", port: 80, local: 2080 }], b: [] });
      const taken = view.takenLocals(store, { slot: "k7q2xa", node: "NODE-B", port: 9000 });
      assert.equal(taken.has(2080), true);
    });
  });

  describe("no opened ports", () => {
    it("is empty", () => {
      assert.equal(view.takenLocals(twoNetStore({ a: [], b: [] })).size, 0);
    });
  });
});

describe("ownRows", () => {
  describe("two networks", () => {
    it("is the published list of the given network's state only", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.main.state.ports.published = [{ port: 8080, name: "web", local: "ok" }];
      store.slots.k7q2xa.state.ports.published = [{ port: 9090, name: "db", local: "ok" }];
      assert.deepEqual(view.ownRows(store, "k7q2xa").map((r) => r.port), [9090]);
      assert.deepEqual(view.ownRows(store, "main").map((r) => r.port), [8080]);
    });

    it("the same port number published in both networks is a row in each, not merged", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.main.state.ports.published = [{ port: 8080, name: "web", local: "ok" }];
      store.slots.k7q2xa.state.ports.published = [{ port: 8080, name: "site", local: "refused" }];
      assert.deepEqual(view.ownRows(store, "main").map((r) => [r.name, r.local]), [["web", "ok"]]);
      assert.deepEqual(view.ownRows(store, "k7q2xa").map((r) => [r.name, r.local]), [["site", "refused"]]);
    });
  });

  describe("a network without published ports", () => {
    it("is an empty list", () => {
      assert.deepEqual(view.ownRows(publishStore({ a: [], b: [] }), "k7q2xa"), []);
    });
  });

  describe("a network whose state has not arrived", () => {
    it("is an empty list", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.k7q2xa.state = null;
      assert.deepEqual(view.ownRows(store, "k7q2xa"), []);
    });
  });
});

describe("ports shared with you across networks", () => {
  const row = { node: "NODE-X", node_name: "bob", online: true, port: 80, name: "", published: true, opened: null };
  const other = { node: "NODE-Y", node_name: "carol", online: true, port: 90, name: "", published: true, opened: null };

  describe("netPortItems(store, slot)", () => {
    it("holds the rows of the given network only", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.main.state.ports.network = [row];
      store.slots.k7q2xa.state.ports.network = [other];
      assert.deepEqual(view.netPortItems(store, "k7q2xa"), [{ slot: "k7q2xa", row: other }]);
      assert.deepEqual(view.netPortItems(store, "main"), [{ slot: "main", row }]);
    });

    it("the same member in both networks is one item in each", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.main.state.ports.network = [row];
      store.slots.k7q2xa.state.ports.network = [row];
      assert.equal(view.netPortItems(store, "main").length, 1);
      assert.equal(view.netPortItems(store, "k7q2xa").length, 1);
    });

    it("an item carries no network name", () => {
      const store = publishStore({ a: [], b: [] });
      store.slots.k7q2xa.state.ports.network = [row];
      assert.deepEqual(Object.keys(view.netPortItems(store, "k7q2xa")[0]).sort(), ["row", "slot"]);
    });

    it("a network with no shared ports is empty", () => {
      assert.deepEqual(view.netPortItems(publishStore({ a: [], b: [] }), "k7q2xa"), []);
    });
  });

  describe("the row key", () => {
    it("includes the slot when given, so the same node and port in two networks do not collide", () => {
      assert.notEqual(netRowKey({ node: "ab", port: 80 }, "main"), netRowKey({ node: "ab", port: 80 }, "k7q2xa"));
      assert.equal(netRowKey({ node: "ab", port: 80 }, "k7q2xa"), "k7q2xa/ab/80");
    });

    it("is node/port without a slot", () => {
      assert.equal(netRowKey({ node: "ab", port: 80 }), "ab/80");
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
  store.state = store.slots.main.state;
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
