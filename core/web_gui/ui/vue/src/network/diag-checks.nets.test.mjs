/* Diagnostics rows across networks: `diagnosticRows(store)` and `netRows(brief, state)`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { diagnosticChecks, diagnosticRows, netRows } from "./diag-checks.ts";
import { acceptOverview } from "../core/overview.ts";
import { createStore } from "../core/store.ts";
import { baseState } from "../testing/section.mjs";

describe("diagnosticRows > one network", () => {
  it("the rows are exactly diagnosticChecks of the state", () => {
    const store = overviewStore([net("main", "friends")]);
    assert.deepEqual(diagnosticRows(store), diagnosticChecks(store.state));
  });

  it("main without a key and one network in another slot — the rows of that network's state", () => {
    const store = overviewStore([net("main", "friends", { keyless: true }), net("k7q2xa", "my devices")]);
    assert.deepEqual(diagnosticRows(store), diagnosticChecks(store.state));
  });

  it("no per-network rows with a label prefix", () => {
    const texts = diagnosticRows(overviewStore([net("main", "friends")])).map((r) => r.text);
    assert.ok(!texts.some((t) => t.startsWith("friends:")), texts.join(" | "));
  });
});

describe("diagnosticRows > two networks", () => {
  describe("process rows", () => {
    it("the node row comes from the main slot", () => {
      const rows = diagnosticRows(twoNetStore());
      assert.equal(pick(rows, "node").text, NET_NAME + " has been running for 3 min");
    });

    it("the proxy row comes from the main slot, not from the other network", () => {
      const text = pick(diagnosticRows(twoNetStore()), "proxy").text;
      assert.ok(text.includes("127.0.0.1:1080"), text);
      assert.ok(!text.includes("2080"), text);
    });

    it("the first row is the node row", () => {
      assert.equal(diagnosticRows(twoNetStore())[0].key, "node");
    });
  });

  describe("a row per network", () => {
    it("each row starts with the network name", () => {
      const texts = diagnosticRows(twoNetStore()).map((r) => r.text);
      assert.equal(texts.filter((t) => t.startsWith("friends: ")).length, 1, texts.join(" | "));
      assert.equal(texts.filter((t) => t.startsWith("my devices: ")).length, 1, texts.join(" | "));
    });

    it("some members online — \"friends: 1 of 2 members online\" (warn)", () => {
      const r = rowWith(diagnosticRows(twoNetStore()), "friends: 1 of 2");
      assert.equal(r.text, "friends: 1 of 2 members online");
      assert.equal(r.level, "warn");
    });

    it("all members online — ok", () => {
      const store = twoNetStore({ peersB: [self(), other("a", true), other("b", true)] });
      const r = rowWith(diagnosticRows(store), "my devices: 2 of 2");
      assert.equal(r.level, "ok");
    });

    it("the second network wholly offline — \"my devices: no connection to the Mesh Town network\" (bad)", () => {
      const store = twoNetStore({ peersB: [self(), other("a", false), other("b", false)] });
      const r = rowWith(diagnosticRows(store), "my devices: no connection");
      assert.equal(r.text, "my devices: no connection to the " + NET_NAME + " network");
      assert.equal(r.level, "bad");
    });

    it("the first network stays as it was when only the second is offline", () => {
      const store = twoNetStore({ peersB: [self(), other("a", false)] });
      assert.equal(rowWith(diagnosticRows(store), "friends: ").text, "friends: 1 of 2 members online");
    });

    it("nobody else in a network — \"my devices: nobody else is here yet\" (warn)", () => {
      const r = rowWith(diagnosticRows(twoNetStore({ peersB: [self()] })), "my devices: ");
      assert.equal(r.text, "my devices: nobody else is here yet");
      assert.equal(r.level, "warn");
    });

    it("the address directory is down in the first network only — a directory row for it alone", () => {
      const store = twoNetStore({ dhtA: false });
      const texts = diagnosticRows(store).map((r) => r.text);
      const dir = "friends: the address directory is not connected — members may not find you";
      assert.ok(texts.includes(dir), texts.join(" | "));
      assert.ok(!texts.some((t) => t.startsWith("my devices:") && t.includes("directory")), texts.join(" | "));
    });

    it("the directory is up in both — no directory rows", () => {
      const texts = diagnosticRows(twoNetStore()).map((r) => r.text);
      assert.ok(!texts.some((t) => t.includes("directory")), texts.join(" | "));
    });

    it("row keys are unique", () => {
      const ks = keys(diagnosticRows(twoNetStore({ dhtA: false })));
      assert.equal(new Set(ks).size, ks.length, ks.join(","));
    });
  });

  describe("the exit row", () => {
    it("direct — \"In the browser, sites open directly\"", () => {
      assert.equal(pick(diagnosticRows(twoNetStore()), "exit").text, "In the browser, sites open directly");
    });

    it("via a member of the network that holds the proxy — \"… via peer-one (friends)\"", () => {
      const route = { state: "exit", exits: [{ id: "bbbb2222", name: "peer-one", active: true }] };
      const store = twoNetStore({ routeA: route });
      assert.equal(pick(diagnosticRows(store), "exit").text, "In the browser, sites open via peer-one (friends)");
    });

    it("the proxy moved to the second network — the exit comes from that slot, named with its label", () => {
      const route = { state: "exit", exits: [{ id: "bbbb2222", name: "peer-one", active: true }] };
      const store = twoNetStore({ proxyIn: "k7q2xa", routeB: route, routeA: { state: "unavailable", exits: [] } });
      assert.equal(pick(diagnosticRows(store), "exit").text, "In the browser, sites open via peer-one (my devices)");
    });

    it("the exit is unreachable — bad, names the member", () => {
      const route = { state: "unavailable", exits: [{ id: "bbbb2222", name: "peer-one", active: false }] };
      const r = pick(diagnosticRows(twoNetStore({ routeA: route })), "exit");
      assert.equal(r.level, "bad");
      assert.ok(r.text.includes("peer-one"), r.text);
    });
  });
});

describe("netRows", () => {
  it("no state — \"<net>: no connection to the Mesh Town network\" (bad)", () => {
    // Only the text and level of the row are checked, not its key.
    const rows = netRows({ id: "k7q2xa", label: "my devices" }, null);
    const r = rows.find((x) => x.text.startsWith("my devices: "));
    assert.equal(r.text, "my devices: no connection to the " + NET_NAME + " network");
    assert.equal(r.level, "bad");
  });
});

function keys(rows) {
  return rows.map((r) => r.key);
}

function pick(rows, key) {
  const list = Array.isArray(rows) ? rows : diagnosticChecks(rows);
  return list.find((r) => r.key === key);
}

const self = () => ({ self: true, online: true, id: "aaaa1111" });
const other = (id, online) => ({ id, online });

// A node with two networks in one overview: A is `main` ("friends"), B is `k7q2xa` ("my devices").
// Options: `peersB`, `dhtA` (is the directory of A up), `routeA`/`routeB`, `proxyIn` (the slot that holds the proxy).
function twoNetStore(o = {}) {
  const peersA = [self(), { id: "bbbb2222", name: "peer-one", online: true }, other("cccc3333", false)];
  const peersB = o.peersB || [self(), { id: "bbbb2222", name: "peer-one", online: true }, other("dddd4444", true)];
  const proxyIn = o.proxyIn || "main";
  return overviewStore([
    net("main", "friends", { peers: peersA, dht: o.dhtA !== false, route: o.routeA, proxy: proxyIn === "main", uptime: 180, socks: "127.0.0.1:1080" }),
    net("k7q2xa", "my devices", { peers: peersB, route: o.routeB, proxy: proxyIn === "k7q2xa", uptime: 9999, socks: "127.0.0.1:2080" }),
  ]);
}

function net(id, label, o = {}) {
  return { id, label, ...o };
}

// A store filled by `acceptOverview` from slots described by `net(...)`.
function overviewStore(specs) {
  const store = createStore();
  const nets = specs.map((s) => brief(s));
  const slots = specs.map((s) => ({ slot: s.id, state: slotState(s), members: null }));
  acceptOverview(store, { api: 2, nets, slots, restart_slots: [] });
  return store;
}

function brief(s) {
  const peers = s.peers || [self(), other("bbbb2222", true)];
  const others = peers.filter((p) => !p.self);
  return {
    id: s.id, label: s.keyless ? null : s.label, id_hex: s.keyless ? null : "key-" + s.id, status: "online", exit_enabled: false,
    proxy: s.proxy ?? s.id === "main", route_state: "direct", exits: [], joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: others.filter((p) => p.online).length, peers_total: others.length,
  };
}

function slotState(s) {
  const peers = s.peers || [self(), other("bbbb2222", true)];
  return baseState({
    boot_id: "boot-" + s.id,
    config_rev: 1,
    node: { id: "node-" + s.id, status: "online", uptime_sec: s.uptime ?? 60 },
    network: s.keyless ? null : { label: s.label, masked: "m", id_hex: "key-" + s.id },
    transport: { relay: { connected: true, url: "r" }, dht: { joined: s.dht !== false }, listen_udp: [] },
    peers,
    route: s.route || { state: "direct", exits: [] },
    socks: { enabled: true, listen: s.socks || "127.0.0.1:1080" },
  });
}

function rowWith(rows, prefix) {
  const found = rows.find((r) => r.text.startsWith(prefix));
  assert.ok(found, "no row starting with \"" + prefix + "\": " + rows.map((r) => r.text).join(" | "));
  return found;
}
