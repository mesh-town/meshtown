/* The "Ports" section across several networks for the two protocols (`ports-view.ts`): taken local numbers are counted per
 * protocol, published ports merge by protocol and number. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createStore } from "../core/store.ts";
import * as view from "./ports-view.ts";

const TCP_DNS = { port: 53, name: "dns", for: "all" };
const UDP_DNS = { port: 53, proto: "udp", name: "dns", for: "all" };

describe("takenLocals", () => {
  const tcp = { node: "ab", port: 80, local: 2053 };
  const udp = { node: "ab", port: 53, proto: "udp", local: 5353 };

  it("by default counts TCP numbers only", () => {
    const taken = view.takenLocals(storeWith([tcp, udp]));
    assert.deepEqual([...taken.keys()], [2053]);
  });

  it("for UDP counts UDP numbers only", () => {
    const taken = view.takenLocals(storeWith([tcp, udp]), undefined, "udp");
    assert.deepEqual([...taken.keys()], [5353]);
  });

  it("TCP 2053 is taken and UDP 2053 is free", () => {
    const store = storeWith([{ node: "ab", port: 80, local: 2053 }]);
    assert.equal(view.takenLocals(store, undefined, "tcp").has(2053), true);
    assert.equal(view.takenLocals(store, undefined, "udp").has(2053), false);
  });

  it("UDP 2053 is taken and TCP 2053 is free", () => {
    const store = storeWith([{ node: "ab", port: 53, proto: "udp", local: 2053 }]);
    assert.equal(view.takenLocals(store, undefined, "udp").has(2053), true);
    assert.equal(view.takenLocals(store, undefined, "tcp").has(2053), false);
  });

  it("the holder of a UDP number is the UDP pair", () => {
    const taken = view.takenLocals(storeWith([tcp, udp]), undefined, "udp");
    assert.equal(taken.get(5353).node, "ab");
    assert.equal(taken.get(5353).port, 53);
    assert.equal(taken.get(5353).slot, "main");
  });

  it("the UDP pair being edited is left out", () => {
    const taken = view.takenLocals(storeWith([tcp, udp]), { slot: "main", node: "ab", port: 53 }, "udp");
    assert.deepEqual([...taken.keys()], []);
  });

  it("the TCP pair of the same node and port is not the edited UDP pair's number", () => {
    const sameTcp = { node: "ab", port: 53, local: 2053 };
    const taken = view.takenLocals(storeWith([sameTcp, udp]), { slot: "main", node: "ab", port: 53 }, "tcp");
    assert.deepEqual([...taken.keys()], [], "the TCP pair itself is excluded");
    const forUdp = view.takenLocals(storeWith([sameTcp, udp]), { slot: "main", node: "ab", port: 53 }, "udp");
    assert.deepEqual([...forUdp.keys()], [], "the UDP pair itself is excluded");
  });
});

describe("mergeOwn", () => {
  it("TCP and UDP of the same number are two rows, each with its protocol", () => {
    const store = ownStore({ a: [TCP_DNS, UDP_DNS], b: [] });
    const rows = view.mergeOwn(store);
    assert.equal(rows.length, 2);
    assert.deepEqual(rows.map((r) => view.protoOf(r)), ["tcp", "udp"]);
    assert.deepEqual(rows.map((r) => r.port), [53, 53]);
  });

  it("UDP published in both networks is one row shared in both", () => {
    const store = ownStore({ a: [UDP_DNS], b: [{ ...UDP_DNS, for: ["ID-PEER-B"] }] });
    const rows = view.mergeOwn(store);
    assert.equal(rows.length, 1);
    assert.equal(view.protoOf(rows[0]), "udp");
    assert.deepEqual(rows[0].shared.map((s) => s.slot), ["main", "k7q2xa"]);
  });

  it("TCP in one network and UDP of the same number in the other are two rows", () => {
    const rows = view.mergeOwn(ownStore({ a: [TCP_DNS], b: [UDP_DNS] }));
    assert.equal(rows.length, 2);
    assert.deepEqual(rows.map((r) => [view.protoOf(r), r.shared.map((s) => s.slot)]), [["tcp", ["main"]], ["udp", ["k7q2xa"]]]);
  });

  it("each row takes the local status of its own protocol", () => {
    const state = [{ port: 53, name: "dns", local: "refused" }, { port: 53, proto: "udp", name: "dns", local: "ok" }];
    const rows = view.mergeOwn(ownStore({ a: [TCP_DNS, UDP_DNS], b: [], state }));
    assert.deepEqual(rows.map((r) => r.local), ["refused", "ok"]);
  });

  it("an entry without proto is a TCP row", () => {
    const rows = view.mergeOwn(ownStore({ a: [TCP_DNS], b: [] }));
    assert.equal(view.protoOf(rows[0]), "tcp");
  });
});

// ---------------------------------------------------------------------------------------------------------------------
// Helpers

function brief(id, label, idHex) {
  return {
    id, label, id_hex: idHex, status: "running", exit_enabled: false, proxy: id === "main",
    route_state: "direct", exits: [], members_voting: 1, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0,
  };
}

function slice(b, config, peers, published = []) {
  const state = { peers, ports: { published, network: [] } };
  return { id: b.id, brief: b, state, members: null, config, configRev: 1, configMeta: null, bootId: null };
}

// Two networks, A = "main", B = "k7q2xa"; `opened` — the opened list of A.
function storeWith(opened) {
  const store = createStore();
  const nets = [brief("main", "friends", "net-aaaa"), brief("k7q2xa", "my devices", "net-bbbb")];
  store.nets = nets;
  store.config = { ports: { published: [], opened } };
  store.slots.main = slice(nets[0], store.config, []);
  store.slots.k7q2xa = slice(nets[1], { ports: { published: [], opened: [] } }, []);
  return store;
}

// Two networks with the given published lists; `state` — the own rows of A.
function ownStore({ a, b, state = [] }) {
  const store = createStore();
  const nets = [brief("main", "friends", "net-aaaa"), brief("k7q2xa", "my devices", "net-bbbb")];
  const peerB = { id: "alice001", id_full: "ID-PEER-B", name: "alice", self: false, online: true };
  store.nets = nets;
  store.config = { ports: { published: a, opened: [] } };
  store.slots.main = slice(nets[0], store.config, [], state);
  store.slots.k7q2xa = slice(nets[1], { ports: { published: b, opened: [] } }, [peerB]);
  return store;
}
