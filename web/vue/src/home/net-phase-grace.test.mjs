/* The "loading network data" deadline and the fifth phase `no_catalog`:
 * `netPhase` and `netStatusText` tell a recently started node (`node.uptime_sec` < 60 s, an unknown
 * uptime is also "recent") from a long-running one. A long-running node with `transport.dht.joined === false` — "the address directory
 * is unavailable" (`no_catalog`) if none of the others is online; "no transport in the snapshot" — "we don't know"
 * (alone/offline), directory connected — alone/offline. Another member online — `online` for any directory and deadline.
 * Tables: other rows × address directory × uptime → expected phase. Words are verbatim. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ALONE_INVITE_TEXT } from "../common/words.ts";
import { netPhase, netStatusText } from "./home-words.ts";

// The node started recently or the deadline is unknown: no directory — "loading".
const STARTING_UPTIMES = [
  { label: "unknown: no node in the snapshot", over: {} },
  { label: "unknown: node is null", over: { node: null } },
  { label: "unknown: node present, no uptime_sec", over: { node: {} } },
  { label: "unknown: uptime_sec is null", over: { node: { uptime_sec: null } } },
  { label: "unknown: uptime_sec is not a number", over: { node: { uptime_sec: "many" } } },
  { label: "0 s", over: { node: { uptime_sec: 0 } } },
  { label: "59 s (the last second of loading)", over: { node: { uptime_sec: 59 } } },
];

// The node has been running for 60 s or longer: the deadline expired.
const SETTLED_UPTIMES = [
  { label: "60 s (boundary: the deadline expired)", over: { node: { uptime_sec: 60 } } },
  { label: "3600 s", over: { node: { status: "online", uptime_sec: 3600 } } },
];

// Expected phase: `starting` / `settled` — by the node's deadline, inside — by the address directory
// (a long-running node × `joined: false` with no others online — `no_catalog`).
const ROWS = [
  {
    others: "no other rows (only your own)",
    peers: () => [self()],
    starting: { noTransport: "loading", notJoined: "loading", joined: "alone" },
    settled: { noTransport: "alone", notJoined: "no_catalog", joined: "alone" },
  },
  {
    others: "one other offline",
    peers: () => [self(), offline()],
    starting: { noTransport: "loading", notJoined: "loading", joined: "offline" },
    settled: { noTransport: "offline", notJoined: "no_catalog", joined: "offline" },
  },
  {
    others: "two others offline",
    peers: () => [self(), offline(), offline()],
    starting: { noTransport: "loading", notJoined: "loading", joined: "offline" },
    settled: { noTransport: "offline", notJoined: "no_catalog", joined: "offline" },
  },
  {
    others: "one other online",
    peers: () => [self(), online()],
    starting: { noTransport: "online", notJoined: "online", joined: "online" },
    settled: { noTransport: "online", notJoined: "online", joined: "online" },
  },
];

const CATALOGS = [
  { key: "noTransport", label: "no transport in the snapshot", over: () => ({}) },
  { key: "notJoined", label: "directory not connected (joined: false)", over: () => ({ transport: dht(false) }) },
  { key: "joined", label: "directory connected (joined: true)", over: () => ({ transport: dht(true, 5) }) },
];

const UPTIME_GROUPS = [
  { group: "starting", uptimes: STARTING_UPTIMES, title: "node started recently" },
  { group: "settled", uptimes: SETTLED_UPTIMES, title: "node running for a long time" },
];

describe("netPhase and the loading deadline", () => {
  for (const row of ROWS) {
    describe(row.others, () => {
      for (const { group, uptimes, title } of UPTIME_GROUPS) {
        for (const uptime of uptimes) {
          describe(`${title}: uptime ${uptime.label}`, () => {
            for (const catalog of CATALOGS) {
              const want = row[group][catalog.key];
              it(`${catalog.label} — ${want}`, () => {
                const got = netPhase(state({ peers: row.peers(), ...catalog.over(), ...uptime.over }));
                assert.equal(got, want);
              });
            }
          });
        }
      }
    });
  }

  describe("60 s boundary (directory not connected: joined: false)", () => {
    it("59 s, no others — loading", () => {
      assert.equal(netPhase(state({ peers: [self()], transport: dht(false), node: { uptime_sec: 59 } })), "loading");
    });

    it("60 s, no others — no_catalog: the deadline expired exactly at 60", () => {
      assert.equal(netPhase(state({ peers: [self()], transport: dht(false), node: { uptime_sec: 60 } })), "no_catalog");
    });

    it("59 s, one other offline — loading", () => {
      assert.equal(netPhase(state({ peers: [self(), offline()], transport: dht(false), node: { uptime_sec: 59 } })), "loading");
    });

    it("60 s, one other offline — no_catalog, not offline", () => {
      assert.equal(netPhase(state({ peers: [self(), offline()], transport: dht(false), node: { uptime_sec: 60 } })), "no_catalog");
    });
  });

  describe("60 s boundary (no transport in the snapshot — directory \"unknown\")", () => {
    it("59 s, no others — loading", () => {
      assert.equal(netPhase(state({ peers: [self()], node: { uptime_sec: 59 } })), "loading");
    });

    it("60 s, no others — alone: nothing is said about the directory", () => {
      assert.equal(netPhase(state({ peers: [self()], node: { uptime_sec: 60 } })), "alone");
    });

    it("60 s, one other offline — offline", () => {
      assert.equal(netPhase(state({ peers: [self(), offline()], node: { uptime_sec: 60 } })), "offline");
    });
  });

  describe("deadline unknown, directory not connected (joined: false)", () => {
    it("no node in the snapshot — loading: no deadline, so no phrase about the directory either", () => {
      assert.equal(netPhase(state({ peers: [self(), offline()], transport: dht(false) })), "loading");
    });

    it("uptime_sec is null — loading", () => {
      const got = netPhase(state({ peers: [self()], transport: dht(false), node: { uptime_sec: null } }));
      assert.equal(got, "loading");
    });
  });

  describe("directory dropped mid-run (long-running node, joined: false)", () => {
    it("there was another member offline — no_catalog: the same words as for \"never connected\"", () => {
      const got = netPhase(state({ peers: [self(), offline()], transport: dht(false), node: { uptime_sec: 7200 } }));
      assert.equal(got, "no_catalog");
    });

    it("a directory record existed before (record.published) — also no_catalog", () => {
      const transport = { ...dht(false), record: { published: true, ttl_sec: 3600 } };
      const got = netPhase(state({ peers: [self()], transport, node: { uptime_sec: 7200 } }));
      assert.equal(got, "no_catalog");
    });
  });

  describe("directory connected (joined: true), long-running node", () => {
    it("7200 s, one other offline — offline", () => {
      const got = netPhase(state({ peers: [self(), offline()], transport: dht(true, 5), node: { uptime_sec: 7200 } }));
      assert.equal(got, "offline");
    });

    it("7200 s, no others — alone", () => {
      const got = netPhase(state({ peers: [self()], transport: dht(true, 5), node: { uptime_sec: 7200 } }));
      assert.equal(got, "alone");
    });
  });

  describe("the snapshot does not say strictly `false` about the directory — \"unknown\", no_catalog is not chosen", () => {
    const unknownCatalogs = [
      { label: "transport present, no dht", transport: {} },
      { label: "dht is null", transport: { dht: null } },
      { label: "dht present, no joined", transport: { dht: { nodes: 0 } } },
    ];

    for (const c of unknownCatalogs) {
      it(`${c.label}, 7200 s, one other offline — offline`, () => {
        const got = netPhase(state({ peers: [self(), offline()], transport: c.transport, node: { uptime_sec: 7200 } }));
        assert.equal(got, "offline");
      });

      it(`${c.label}, 7200 s, no others — alone`, () => {
        const got = netPhase(state({ peers: [self()], transport: c.transport, node: { uptime_sec: 7200 } }));
        assert.equal(got, "alone");
      });
    }
  });

  describe("another member online — online for any directory and deadline", () => {
    for (const catalog of CATALOGS) {
      for (const sec of [0, 59, 60, 7200]) {
        it(`${catalog.label}, uptime ${sec} s`, () => {
          const got = netPhase(state({ peers: [self(), offline(), online()], ...catalog.over(), node: { uptime_sec: sec } }));
          assert.equal(got, "online");
        });
      }
    }
  });

  describe("no snapshot at all", () => {
    it("null — loading: deadline unknown, no directory", () => assert.equal(netPhase(null), "loading"));
    it("undefined — loading", () => assert.equal(netPhase(undefined), "loading"));
  });
});

// Home's words take the phase from `netPhase`: a long-running node without a directory speaks by what it knows (no transport) or
// "directory unavailable" (joined: false), a recent one — "Loading…". The full words of the fifth phase are in `no-catalog.test.mjs`.
describe("netStatusText and the loading deadline", () => {
  const OFFLINE_ONE = "The other member is offline — looking for them through the address directory.";
  const OFFLINE_ALL = "All other members are offline — looking for them through the address directory.";
  const NO_CATALOG_ALONE = "Mesh Town can't reach the address directory that helps members find each other. Invite members anyway: the invite contains your address, so they can connect without it.";
  const NO_CATALOG_ONE = "The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
  const NO_CATALOG_ALL = "All other members are offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
  const LOADING = "Loading network data…";
  // `settled` — the text of a long-running node by the directory key: noTransport — "unknown", notJoined — "unavailable", joined — connected.
  const snapshots = [
    { others: "no others", peers: () => [self()], settled: { noTransport: ALONE_INVITE_TEXT, notJoined: NO_CATALOG_ALONE, joined: ALONE_INVITE_TEXT } },
    { others: "one other offline", peers: () => [self(), offline()], settled: { noTransport: OFFLINE_ONE, notJoined: NO_CATALOG_ONE, joined: OFFLINE_ONE } },
    { others: "two others offline", peers: () => [self(), offline(), offline()], settled: { noTransport: OFFLINE_ALL, notJoined: NO_CATALOG_ALL, joined: OFFLINE_ALL } },
  ];
  const noCatalog = CATALOGS.filter((c) => c.key !== "joined");

  for (const snap of snapshots) {
    describe(`${snap.others}, no directory`, () => {
      for (const catalog of noCatalog) {
        describe(catalog.label, () => {
          const settled = snap.settled[catalog.key];

          it("node running for a long time (3600 s) — text by the node's knowledge", () => {
            const got = netStatusText(state({ peers: snap.peers(), ...catalog.over(), node: { uptime_sec: 3600 } }));
            assert.equal(got, settled);
          });

          it("node running for exactly 60 s — already the text by the node's knowledge", () => {
            const got = netStatusText(state({ peers: snap.peers(), ...catalog.over(), node: { uptime_sec: 60 } }));
            assert.equal(got, settled);
          });

          it("node started 10 s ago — \"Loading network data…\"", () => {
            const got = netStatusText(state({ peers: snap.peers(), ...catalog.over(), node: { uptime_sec: 10 } }));
            assert.equal(got, LOADING);
          });

          it("node running for 59 s — still \"Loading network data…\"", () => {
            const got = netStatusText(state({ peers: snap.peers(), ...catalog.over(), node: { uptime_sec: 59 } }));
            assert.equal(got, LOADING);
          });

          it("deadline unknown (no node) — \"Loading network data…\"", () => {
            const got = netStatusText(state({ peers: snap.peers(), ...catalog.over() }));
            assert.equal(got, LOADING);
          });
        });
      }
    });

    describe(`${snap.others}, directory connected`, () => {
      it("node started 10 s ago — loading does not hold: text by the node's knowledge", () => {
        const got = netStatusText(state({ peers: snap.peers(), transport: dht(true, 5), node: { uptime_sec: 10 } }));
        assert.equal(got, snap.settled.joined);
      });

      it("node running for a long time (7200 s) — the usual text, says nothing about an unavailable directory", () => {
        const got = netStatusText(state({ peers: snap.peers(), transport: dht(true, 5), node: { uptime_sec: 7200 } }));
        assert.equal(got, snap.settled.joined);
      });
    });
  }

  describe("another member online", () => {
    for (const catalog of noCatalog) {
      it(`${catalog.label}, node running for a long time — no status (null)`, () => {
        const got = netStatusText(state({ peers: [self(), online()], ...catalog.over(), node: { uptime_sec: 3600 } }));
        assert.equal(got, null);
      });
    }
  });

  describe("words", () => {
    it("long-running node, no directory in the snapshot: no words about \"loading\" or \"connecting\"", () => {
      const got = netStatusText(state({ peers: [self(), offline()], node: { uptime_sec: 3600 } }));
      assert.doesNotMatch(got, /Loading|connecting|Looking for members/);
    });

    it("long-running node, directory not connected: no words about \"loading\" and no promise of \"searching via the shared directory\"", () => {
      const got = netStatusText(state({ peers: [self(), offline()], transport: dht(false), node: { uptime_sec: 3600 } }));
      assert.doesNotMatch(got, /Loading|connecting|Looking for members|searching via the shared address directory/);
    });
  });
});

function self() {
  return { self: true, online: true };
}

// A known other member who is offline now (a row from the address cache).
function offline() {
  return { self: false, online: false };
}

// Another member who is online.
function online() {
  return { self: false, online: true };
}

function dht(joined, nodes = 0) {
  return { dht: { joined, nodes } };
}

function state(over = {}) {
  return { network: { label: "home", id_hex: "n" }, peers: [], ...over };
}
