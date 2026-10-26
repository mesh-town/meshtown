/* Diagnostics status lights: `diagnosticChecks(state)` and the "About" formatters. The output reads
 * as a phrase: `diagnosticChecks > peers > two_of_three_online`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { diagnosticChecks, formatUptime, formatMib, LEVEL_ICON } from "./diag-checks.ts";

describe("diagnosticChecks > node", () => {
  it("online 4320 s — ok \"Mesh Town has been running for 1 h 12 min\"", () => {
    const r = pick(snapshot({ node: { status: "online", uptime_sec: 4320 } }), "node");
    assert.equal(r.level, "ok");
    assert.equal(r.text, NET_NAME + " has been running for 1 h 12 min");
  });

  it("starting — warn", () => {
    const r = pick(snapshot({ node: { status: "starting" } }), "node");
    assert.equal(r.level, "warn");
    assert.equal(r.text, NET_NAME + " is starting…");
  });

  it("stopping — warn", () => {
    const r = pick(snapshot({ node: { status: "stopping" } }), "node");
    assert.equal(r.level, "warn");
    assert.equal(r.text, NET_NAME + " is stopping…");
  });

  it("online without uptime_sec — \"Mesh Town is running\" without a time", () => {
    const r = pick(snapshot({ node: { status: "online" } }), "node");
    assert.equal(r.level, "ok");
    assert.equal(r.text, NET_NAME + " is running");
  });

  it("online for less than an hour — minutes only", () => {
    const r = pick(snapshot({ node: { status: "online", uptime_sec: 300 } }), "node");
    assert.equal(r.text, NET_NAME + " has been running for 5 min");
  });
});

describe("diagnosticChecks > no network", () => {
  it("the rows node, network (bad) and proxy remain", () => {
    const rows = diagnosticChecks(snapshot({ network: null }));
    assert.deepEqual(keys(rows), ["node", "network", "proxy"]);
    assert.equal(pick(rows, "network").level, "bad");
  });

  it("directory, peers, exit are not shown", () => {
    const ks = keys(diagnosticChecks(snapshot({ network: null })));
    for (const k of ["directory", "peers", "exit"]) assert.ok(!ks.includes(k), k);
  });
});

describe("diagnosticChecks > network", () => {
  it("label present — \"You're in the network \\\"label\\\"\"", () => {
    assert.equal(pick(snapshot(), "network").text, "You're in the network \"home\"");
  });

  it("label null — \"You're in a Mesh Town network\"", () => {
    const r = pick(snapshot({ network: { label: null, masked: "m", id_hex: "i" } }), "network");
    assert.equal(r.level, "ok");
    assert.equal(r.text, "You're in a Mesh Town network");
  });
});

describe("diagnosticChecks > directory", () => {
  it("dht.joined — ok", () => {
    assert.equal(pick(snapshot(), "directory").level, "ok");
  });

  it("record read from the DHT (read_sec) — ok, even if joined is not there yet", () => {
    const st = snapshot({ transport: { dht: { joined: false }, record: { read_sec: 7 } } });
    assert.equal(pick(st, "directory").level, "ok");
  });

  it("dht not connected — warn", () => {
    const st = snapshot({ transport: { ...snapshot().transport, dht: { joined: false } } });
    assert.equal(pick(st, "directory").level, "warn");
  });
});

describe("diagnosticChecks > without a relay row", () => {
  it("relay is not shown in any relay state", () => {
    for (const relay of [{ connected: true, url: "r" }, { connected: false, url: null }]) {
      const st = snapshot({ transport: { relay, dht: { joined: true } } });
      assert.ok(!keys(diagnosticChecks(st)).includes("relay"));
    }
  });
});

describe("diagnosticChecks > peers", () => {
  it("only your own row — warn \"Nobody else…\"", () => {
    const r = pick(snapshot({ peers: [{ self: true, online: true }] }), "peers");
    assert.equal(r.level, "warn");
    assert.match(r.text, /^Nobody else/);
  });

  it("0 of 2 online — bad", () => {
    const peers = [{ self: true, online: true }, { id: "a", online: false }, { id: "b", online: false }];
    const r = pick(snapshot({ peers }), "peers");
    assert.equal(r.level, "bad");
    assert.equal(r.text, "No members are online");
  });

  it("1 of 2 online — warn (partial availability), href #peers", () => {
    const peers = [{ self: true, online: true }, { id: "a", online: true }, { id: "b", online: false }];
    const r = pick(snapshot({ peers }), "peers");
    assert.equal(r.level, "warn");
    assert.equal(r.text, "1 of 2 members online");
    assert.equal(r.href, "#peers");
  });

  it("2 of 2 online — ok", () => {
    const peers = [{ self: true, online: true }, { id: "a", online: true }, { id: "b", online: true }];
    assert.equal(pick(snapshot({ peers }), "peers").level, "ok");
  });

  it("3 of 3 online — ok, href #peers, your own row is not counted", () => {
    const peers = [
      { self: true, online: true },
      { id: "a", online: true },
      { id: "b", online: true },
      { id: "c", online: true },
    ];
    const r = pick(snapshot({ peers }), "peers");
    assert.equal(r.level, "ok");
    assert.equal(r.text, "3 of 3 members online");
    assert.equal(r.href, "#peers");
  });

  it("plural by the number of members: 1, 21, 5", () => {
    const text = (n) => pick(snapshot({ peers: Array.from({ length: n }, (_, i) => ({ id: "p" + i, online: true })) }), "peers").text;
    assert.equal(text(1), "1 of 1 member online");
    assert.equal(text(21), "21 of 21 members online");
    assert.equal(text(5), "5 of 5 members online");
  });
});

describe("diagnosticChecks > exit", () => {
  it("direct — ok \"In the browser, sites open directly\" (NET_NAME)", () => {
    const r = pick(snapshot({ route: { state: "direct", exits: [] } }), "exit");
    assert.equal(r.level, "ok");
    assert.equal(r.text, "In the browser, sites open directly");
  });

  it("exit — the name of the exits[] item with active:true, not the first", () => {
    const exits = [{ id: "a", name: "First" }, { id: "b", name: "Active", active: true }];
    const r = pick(snapshot({ route: { state: "exit", exits } }), "exit");
    assert.equal(r.level, "ok");
    assert.equal(r.text, "In the browser, sites open via Active");
  });

  it("unavailable — the name of the first of exits, bad", () => {
    const exits = [{ id: "a", name: "First" }, { id: "b", name: "Second" }];
    const r = pick(snapshot({ route: { state: "unavailable", exits } }), "exit");
    assert.equal(r.level, "bad");
    assert.equal(r.text, "First is offline — sites may not open or may open directly");
  });

  it("exit whose name is shared with another member — \"name (id)\" tells them apart", () => {
    const exits = [{ id: "b", name: "Active", active: true }];
    const peers = [{ id: "b", name: "Active", online: true }, { id: "c", name: "active ", online: true }];
    const r = pick(snapshot({ route: { state: "exit", exits }, peers }), "exit");
    assert.equal(r.text, "In the browser, sites open via Active (b)");
  });

  it("exit without active — \"the chosen member\"", () => {
    const r = pick(snapshot({ route: { state: "exit", exits: [{ id: "a" }] } }), "exit");
    assert.equal(r.text, "In the browser, sites open via the chosen member");
  });

  it("no route — \"How sites open — no data\"", () => {
    const r = pick(snapshot({ route: null }), "exit");
    assert.equal(r.text, "How sites open — no data");
  });

  it("unavailable with empty exits — \"The chosen member is offline…\"", () => {
    const r = pick(snapshot({ route: { state: "unavailable", exits: [] } }), "exit");
    assert.equal(r.text, "The chosen member is offline — sites may not open or may open directly");
  });
});

describe("diagnosticChecks > proxy", () => {
  it("socks enabled — ok with the address", () => {
    assert.equal(pick(snapshot(), "proxy").text, "The browser proxy is running on 127.0.0.1:1080");
  });

  it("socks disabled — warn", () => {
    const r = pick(snapshot({ socks: { enabled: false } }), "proxy");
    assert.equal(r.level, "warn");
    assert.equal(r.text, "The browser proxy is off");
  });
});

describe("diagnosticChecks > edge cases", () => {
  it("empty snapshot {} — no exception, rows none \"no data\"", () => {
    const rows = diagnosticChecks({});
    assert.equal(pick(rows, "node").level, "none");
    assert.equal(pick(rows, "node").text, NET_NAME + " — no data");
    assert.match(pick(rows, "proxy").text, /no data/);
  });

  it("{ node: {} } — no exception, node none, \"Mesh Town — no data\"", () => {
    const rows = diagnosticChecks({ node: {} });
    assert.equal(pick(rows, "node").level, "none");
    assert.equal(pick(rows, "node").text, NET_NAME + " — no data");
  });

  it("a network exists, transport/route/socks do not — rows none \"no data\"", () => {
    const rows = diagnosticChecks({ node: { status: "online" }, network: { label: "x" }, peers: [] });
    for (const k of ["directory", "exit", "proxy"]) {
      assert.equal(pick(rows, k).level, "none", k);
      assert.match(pick(rows, k).text, /no data/, k);
    }
  });

  it("every row has a level out of the four allowed, and there is an icon for it", () => {
    for (const r of diagnosticChecks(snapshot())) assert.ok(LEVEL_ICON[r.level], r.key);
    assert.deepEqual(LEVEL_ICON, { ok: "✓", warn: "⚠", bad: "✗", none: "—" });
  });

  it("no text contains a bare \"via the network\"", () => {
    const states = [
      snapshot(),
      snapshot({ route: { state: "exit", exits: [{ id: "b", active: true }] } }),
      snapshot({ route: { state: "unavailable", exits: [] } }),
      snapshot({ network: null }),
      {},
    ];
    for (const st of states) {
      for (const r of diagnosticChecks(st)) assert.doesNotMatch(r.text, /via the network(?! Mesh Town)/);
    }
  });
});

describe("formatUptime", () => {
  it("zero seconds — 0 h 0 min", () => {
    assert.equal(formatUptime(0), "0 h 0 min");
  });

  it("counts hours and minutes separately", () => {
    assert.equal(formatUptime(3661), "1 h 1 min");
  });

  it("null — 0 h 0 min", () => {
    assert.equal(formatUptime(null), "0 h 0 min");
  });
});

describe("formatMib", () => {
  it("1024 KiB — \"1.0 MiB\"", () => {
    assert.equal(formatMib(1024), "1.0 MiB");
  });

  it("0 KiB — \"0.0 MiB\"", () => {
    assert.equal(formatMib(0), "0.0 MiB");
  });

  it("rounds to one digit: 1536 KiB — \"1.5 MiB\"", () => {
    assert.equal(formatMib(1536), "1.5 MiB");
  });

  it("null — «—»", () => {
    assert.equal(formatMib(null), "—");
  });
});

function keys(rows) {
  return rows.map((r) => r.key);
}

function pick(rows, key) {
  const list = Array.isArray(rows) ? rows : diagnosticChecks(rows);
  return list.find((r) => r.key === key);
}

function snapshot(over = {}) {
  return {
    node: { status: "online", uptime_sec: 60 },
    network: { label: "home", masked: "mesh2-xx", id_hex: "aa11" },
    transport: { relay: { connected: true, url: "r" }, dht: { joined: true }, listen_udp: [] },
    peers: [{ self: true, online: true }, { id: "a", online: true }],
    route: { state: "direct", exits: [] },
    socks: { enabled: true, listen: "127.0.0.1:1080" },
    ...over,
  };
}
