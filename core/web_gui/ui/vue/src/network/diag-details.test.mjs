/* Diagnostics details: the connection table, "About", events, the "Connect manually" response. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { transportRows, aboutLines, eventRows, peerAddrResultText } from "./diag-details.ts";

describe("transportRows", () => {
  it("four rows, no \"relay server\" row", () => {
    const rows = transportRows(transport());
    assert.deepEqual(
      rows.map((r) => r.label),
      ["NAT", "port for connections", "address directory", "member record"],
    );
  });

  it("NAT — a string as is", () => {
    assert.equal(valueOf(transport({ nat: "undetermined" }), "NAT"), "undetermined");
  });

  it("ports comma-separated", () => {
    assert.equal(valueOf(transport({ listen_udp: ["1.2.3.4:5", "6.7.8.9:1"] }), "port for connections"), "1.2.3.4:5, 6.7.8.9:1");
  });

  it("no DHT — \"no entry point\"", () => {
    assert.equal(valueOf(transport({ dht: { joined: false } }), "address directory"), "no entry point");
  });

  it("DHT present — \"connected, directory nodes: N; record refreshed S s ago\"", () => {
    const t = transport({ dht: { joined: true, nodes: 7, last_publish_sec: 12 } });
    assert.equal(valueOf(t, "address directory"), "connected, directory nodes: 7; record refreshed 12 s ago");
  });

  it("DHT present, 0 nodes — \"connected\" without \"directory nodes\"", () => {
    const t = transport({ dht: { joined: true, nodes: 0, last_publish_sec: 4 } });
    const v = valueOf(t, "address directory");
    assert.equal(v, "connected; record refreshed 4 s ago");
    assert.doesNotMatch(v, /directory nodes/);
  });

  it("DHT present, last_publish_sec null — without \"null\" and without a tail", () => {
    const t = transport({ dht: { joined: true, nodes: 7, last_publish_sec: null } });
    const v = valueOf(t, "address directory");
    assert.equal(v, "connected, directory nodes: 7");
    assert.doesNotMatch(v, /null|refreshed/);
  });

  it("record published — \"published, valid for another N s\"", () => {
    assert.equal(valueOf(transport({ record: { published: true, ttl_sec: 300 } }), "member record"), "published, valid for another 300 s");
  });

  it("record published, no ttl — just \"published\"", () => {
    assert.equal(valueOf(transport({ record: { published: true } }), "member record"), "published");
  });

  it("record not published", () => {
    assert.equal(valueOf(transport({ record: { published: false } }), "member record"), "not published");
  });

  it("transport did not arrive — no exception, four rows", () => {
    assert.equal(transportRows(undefined).length, 4);
  });
});

describe("aboutLines", () => {
  it("two classic lines: address and version; memory and uptime", () => {
    const node = { web_addr: "127.0.0.1:7777", version: "0.9.1", rss_kib: 14643, uptime_sec: 3661 };
    assert.deepEqual(aboutLines(node), [
      NET_NAME + " on this computer · 127.0.0.1:7777 · version 0.9.1",
      "Memory 14.3 MiB · uptime 1 h 1 min",
    ]);
  });

  it("node is empty — \"—\" in place of the address, version and memory", () => {
    const [a, b] = aboutLines({});
    assert.equal(a, NET_NAME + " on this computer · — · version —");
    assert.match(b, /^Memory — · uptime 0 h 0 min$/);
  });

  it("no node at all — no exception", () => {
    assert.equal(aboutLines(undefined).length, 2);
  });
});

describe("eventRows", () => {
  it("not an array — an empty list", () => {
    assert.deepEqual(eventRows(undefined), []);
    assert.deepEqual(eventRows(null), []);
    assert.deepEqual(eventRows({}), []);
  });

  it("seq, tag and text are kept, the time is in the log format", () => {
    const rows = eventRows([{ seq: 4124, t: 240.6, tag: "member", text: "- c03b: not responding" }]);
    assert.deepEqual(rows, [{ seq: 4124, time: "+  240.6s", tag: "member", text: "- c03b: not responding" }]);
  });

  it("the order of rows is the same", () => {
    const rows = eventRows([{ seq: 1, t: 1, tag: "a", text: "x" }, { seq: 2, t: 2, tag: "b", text: "y" }]);
    assert.deepEqual(rows.map((r) => r.seq), [1, 2]);
  });
});

describe("peerAddrResultText", () => {
  it("no rejections — \"Addresses added: N, rejected: 0\"", () => {
    assert.equal(peerAddrResultText({ added: 2, rejected: [] }), "Addresses added: 2, rejected: 0");
  });

  it("with rejections — reasons in parentheses joined by \"; \"", () => {
    const res = { added: 1, rejected: [{ line: 2, message: "no port" }, { line: 3, message: "not an address" }] };
    assert.equal(peerAddrResultText(res), "Addresses added: 1, rejected: 2 (2: no port; 3: not an address)");
  });
});

function valueOf(t, label) {
  return transportRows(t).find((r) => r.label === label).value;
}

function transport(over = {}) {
  return {
    relay: { connected: true, url: "wss://r.example" },
    nat: "open",
    listen_udp: ["0.0.0.0:4000"],
    dht: { joined: true, nodes: 3, last_publish_sec: 5 },
    record: { published: true, ttl_sec: 60 },
    ...over,
  };
}
