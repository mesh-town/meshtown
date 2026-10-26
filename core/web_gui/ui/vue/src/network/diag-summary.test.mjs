/* Summary for help: the log request path and the whitelist of `buildSummary` fields. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { summaryLogPath, buildSummary } from "./diag-summary.ts";
import { NET_NAME } from "../common/words.ts";
import { diagnosticChecks } from "./diag-checks.ts";

const SECRETS = {
  masked: "mesh2-MASKEDSECRET",
  id_hex: "IDHEXSECRET0123",
  self_address: "203.0.113.77:41414",
  listen: "198.51.100.9:50505",
  id_full: "IDFULLSECRET9876",
};

describe("summaryLogPath", () => {
  it("last_seq 4127 — after=4077, limit=50", () => {
    assert.equal(summaryLogPath({ log: { last_seq: 4127 } }), "/api/log?after=4077&limit=50");
  });

  it("last_seq 10 — after=0", () => {
    assert.equal(summaryLogPath({ log: { last_seq: 10 } }), "/api/log?after=0&limit=50");
  });

  it("no log — null (a request without after would return the oldest lines)", () => {
    assert.equal(summaryLogPath({}), null);
    assert.equal(summaryLogPath(undefined), null);
  });
});

describe("buildSummary > whitelist", () => {
  it("the text has no mask, id_hex, addresses or id_full", () => {
    const text = summaryOf(secretState());
    for (const [name, value] of Object.entries(SECRETS)) {
      assert.ok(!text.includes(value), `the summary leaked the field ${name}: ${value}`);
    }
  });

  it("the invite and the key are not included", () => {
    assert.doesNotMatch(summaryOf(secretState()), /mesh-invite[12]-|mesh2-/);
  });

  it("the version is the first line, then the status-light rows with icons", () => {
    const text = summaryOf(secretState());
    const lines = text.split("\n");
    assert.equal(lines[0], "Mesh Town 1.2.3");
    assert.ok(lines.includes("✓ You're in the network \"home\""), text);
    assert.ok(lines.some((l) => l.startsWith("✓ " + NET_NAME + " has been running")), text);
  });

  it("after the status lights — an empty line and \"Latest log lines:\"", () => {
    const lines = summaryOf(secretState()).split("\n");
    const at = lines.indexOf("Latest log lines:");
    assert.ok(at > 0);
    assert.equal(lines[at - 1], "");
  });
});

describe("buildSummary > log", () => {
  it("log lines are in the log format \"+ time [tag] text\"", () => {
    const text = summaryOf(secretState(), { lines: [{ seq: 1, t: 12.5, tag: "gossip", text: "peer +" }] });
    assert.ok(text.endsWith("+   12.5s [gossip] peer +"), text);
  });

  it("log unavailable — the line \"log unavailable: …\", the status lights in place", () => {
    const text = summaryOf(secretState(), { logError: "The node responded with an error 500" });
    assert.ok(text.endsWith("log unavailable: The node responded with an error 500"), text);
    assert.match(text, /✓ You're in the network/);
  });
});

function summaryOf(state, journal = { lines: [] }) {
  return buildSummary({ state, checks: diagnosticChecks(state), ...journal });
}

function secretState() {
  return {
    node: { status: "online", uptime_sec: 60, version: "1.2.3", id_full: SECRETS.id_full },
    network: { label: "home", masked: SECRETS.masked, id_hex: SECRETS.id_hex },
    transport: {
      relay: { connected: true, url: "wss://r.example" },
      dht: { joined: true },
      self_addresses: [SECRETS.self_address],
      listen_udp: [SECRETS.listen],
    },
    peers: [{ self: true, online: true }, { id: "a", online: true }],
    route: { state: "direct", exits: [] },
    socks: { enabled: true, listen: "127.0.0.1:1080" },
    log: { tail: [], last_seq: 100 },
  };
}

describe("buildSummary > log cleaning", () => {
  it("the token, network and node ids, addresses from log lines do not get into the summary", () => {
    const state = secretState();
    const lines = [
      { seq: 1, t: 1, tag: "web", text: "Mesh Town page: http://127.0.0.1:7070/?token=tok123" },
      { seq: 2, t: 2, tag: "node", text: `network ${SECRETS.id_hex}, node ${SECRETS.id_full} [2001:db8::1]:5` },
    ];
    const text = summaryOf(state, { lines }).split("Latest log lines:")[1];
    for (const bad of ["tok123", "127.0.0.1", SECRETS.id_hex, SECRETS.id_full, "2001:db8"]) {
      assert.ok(!text.includes(bad), `the summary still has ${bad}`);
    }
    assert.doesNotMatch(text, /token=(?!‹hidden›)/);
  });
});
