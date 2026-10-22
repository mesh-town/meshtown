/* The "loading network data" period in the header tail and "members can't find you automatically" (
 * `othersOnlineText(state)`). The address directory is not connected and the node started recently (`node.uptime_sec` < 60 s,
 * unknown also counts as "recently") — "loading network data…"; a long-running node (≥ 60 s) with `transport.dht.joined === false`,
 * with no other rows — "members can't find you automatically", and with known but offline ones — "offline" (Home names the directory); a long-running node without transport in the snapshot speaks from
 * what the node knows. An online member is always "N members online". */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { othersOnlineText } from "./status-line.ts";

const LOADING = "loading network data…";

const NO_CATALOG = "members can't find you automatically";

// What the header says when there is no address directory, by node uptime: `noTransport` — the snapshot has no transport
// ("unknown"), `notJoined` — the node reported joined: false.
const NO_CATALOG_ROWS = [
  {
    others: "no other rows (only the own one)",
    peers: () => [SELF],
    starting: { noTransport: LOADING, notJoined: LOADING },
    settled: { noTransport: "invite members", notJoined: NO_CATALOG },
  },
  {
    others: "one other member offline",
    peers: () => [SELF, OFFLINE],
    starting: { noTransport: LOADING, notJoined: LOADING },
    settled: { noTransport: "the other member is offline", notJoined: "the other member is offline" },
  },
  {
    others: "two other members offline",
    peers: () => [SELF, OFFLINE, OFFLINE],
    starting: { noTransport: LOADING, notJoined: LOADING },
    settled: { noTransport: "all other members are offline", notJoined: "all other members are offline" },
  },
  {
    others: "one other member online",
    peers: () => [SELF, ONLINE],
    starting: { noTransport: "1 member online", notJoined: "1 member online" },
    settled: { noTransport: "1 member online", notJoined: "1 member online" },
  },
];

const CATALOGS = [
  { key: "noTransport", label: "no transport in the snapshot", over: () => ({}) },
  { key: "notJoined", label: "directory not connected (joined: false)", over: () => ({ transport: dht(false) }) },
];

const STARTING_UPTIMES = [
  { label: "unknown: no node", over: {} },
  { label: "unknown: node is null", over: { node: null } },
  { label: "unknown: node without uptime_sec", over: { node: {} } },
  { label: "unknown: uptime_sec is not a number", over: { node: { uptime_sec: null } } },
  { label: "0 s", over: { node: { uptime_sec: 0 } } },
  { label: "10 s", over: { node: { uptime_sec: 10 } } },
  { label: "59 s", over: { node: { uptime_sec: 59 } } },
];

const SETTLED_UPTIMES = [
  { label: "60 s (boundary)", over: { node: { uptime_sec: 60 } } },
  { label: "3600 s", over: { node: { uptime_sec: 3600 } } },
];

const GROUPS = [
  { group: "starting", uptimes: STARTING_UPTIMES, title: "node started recently" },
  { group: "settled", uptimes: SETTLED_UPTIMES, title: "node has been running for a long time" },
];

describe("othersOnlineText and the loading period", () => {
  for (const row of NO_CATALOG_ROWS) {
    describe(row.others, () => {
      for (const { group, uptimes, title } of GROUPS) {
        for (const uptime of uptimes) {
          describe(`${title}: uptime ${uptime.label}`, () => {
            for (const catalog of CATALOGS) {
              const want = row[group][catalog.key];
              it(`${catalog.label} — «${want}»`, () => {
                const got = othersOnlineText({ peers: row.peers(), ...catalog.over(), ...uptime.over });
                assert.equal(got, want);
              });
            }
          });
        }
      }
    });
  }

  describe("directory connected — loading does not hold at any uptime", () => {
    const joinedRows = [
      { others: "no other members", peers: () => [SELF], want: "invite members" },
      { others: "one other member offline", peers: () => [SELF, OFFLINE], want: "the other member is offline" },
      { others: "two other members offline", peers: () => [SELF, OFFLINE, OFFLINE], want: "all other members are offline" },
      { others: "one other member online", peers: () => [SELF, ONLINE], want: "1 member online" },
    ];

    for (const row of joinedRows) {
      for (const sec of [0, 10, 59, 60, 3600, 7200]) {
        it(`${row.others}, uptime ${sec} s — "${row.want}"`, () => {
          const got = othersOnlineText({ peers: row.peers(), transport: dht(true), node: { uptime_sec: sec } });
          assert.equal(got, row.want);
        });
      }
    }
  });

  describe("60 s boundary, directory not connected (joined: false): 59 — still loading, 60 — already \"members can't find you automatically\"", () => {
    it("59 s, no other members — \"loading network data…\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF], transport: dht(false), node: { uptime_sec: 59 } }), LOADING);
    });

    it("60 s, no other members — \"members can't find you automatically\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF], transport: dht(false), node: { uptime_sec: 60 } }), NO_CATALOG);
    });

    it("59 s, another member offline — \"loading network data…\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], transport: dht(false), node: { uptime_sec: 59 } }), LOADING);
    });

    it("60 s, another member offline — \"the other member is offline\" (Home names the directory)", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], transport: dht(false), node: { uptime_sec: 60 } }), "the other member is offline");
    });
  });

  describe("60 s boundary, no transport in the snapshot: the header says nothing about the directory", () => {
    it("59 s, no other members — \"loading network data…\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF], node: { uptime_sec: 59 } }), LOADING);
    });

    it("60 s, no other members — \"invite members\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF], node: { uptime_sec: 60 } }), "invite members");
    });

    it("60 s, another member offline — \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], node: { uptime_sec: 60 } }), "the other member is offline");
    });
  });

  describe("uptime unknown, directory not connected (joined: false)", () => {
    it("no node in the snapshot — \"loading network data…\": no uptime, no directory phrase", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], transport: dht(false) }), LOADING);
    });

    it("uptime_sec is null — \"loading network data…\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF], transport: dht(false), node: { uptime_sec: null } }), LOADING);
    });
  });

  describe("the directory dropped mid-run (long-running node, joined: false)", () => {
    it("there was an offline member — \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], transport: dht(false), node: { uptime_sec: 7200 } }), "the other member is offline");
    });

    it("two other members offline — \"all other members are offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE, OFFLINE], transport: dht(false), node: { uptime_sec: 7200 } }), "all other members are offline");
    });

    it("the directory record existed before (record.published) — also \"members can't find you automatically\"", () => {
      const transport = { ...dht(false), record: { published: true, ttl_sec: 3600 } };
      assert.equal(othersOnlineText({ peers: [SELF], transport, node: { uptime_sec: 7200 } }), NO_CATALOG);
    });

    it("another member online — \"1 member online\": the directory does not matter", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE, ONLINE], transport: dht(false), node: { uptime_sec: 7200 } }), "1 member online");
    });
  });

  describe("the snapshot does not say a strict `false` about the directory — \"unknown\", \"members can't find you automatically\" is not said", () => {
    const unknownCatalogs = [
      { label: "transport present, no dht", transport: {} },
      { label: "dht is null", transport: { dht: null } },
      { label: "dht present, no joined", transport: { dht: { nodes: 0 } } },
    ];

    for (const c of unknownCatalogs) {
      it(`${c.label}, 7200 s, another member offline — "the other member is offline"`, () => {
        assert.equal(othersOnlineText({ peers: [SELF, OFFLINE], transport: c.transport, node: { uptime_sec: 7200 } }), "the other member is offline");
      });

      it(`${c.label}, 7200 s, no other members — "invite members"`, () => {
        assert.equal(othersOnlineText({ peers: [SELF], transport: c.transport, node: { uptime_sec: 7200 } }), "invite members");
      });
    }
  });

  describe("snapshot edge cases for a long-running node without a directory", () => {
    it("peers not set — \"invite members\": no other rows", () => {
      assert.equal(othersOnlineText({ node: { uptime_sec: 3600 } }), "invite members");
    });

    it("peers empty — \"invite members\"", () => {
      assert.equal(othersOnlineText({ peers: [], node: { uptime_sec: 3600 } }), "invite members");
    });

    it("an offline member without the self field is also another member: \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, { online: false }], node: { uptime_sec: 3600 } }), "the other member is offline");
    });

    it("peers not set, joined: false — \"members can't find you automatically\"", () => {
      assert.equal(othersOnlineText({ transport: dht(false), node: { uptime_sec: 3600 } }), NO_CATALOG);
    });

    it("peers empty, joined: false — \"members can't find you automatically\"", () => {
      assert.equal(othersOnlineText({ peers: [], transport: dht(false), node: { uptime_sec: 3600 } }), NO_CATALOG);
    });

    it("an offline member without the self field, joined: false — also another member: \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, { online: false }], transport: dht(false), node: { uptime_sec: 3600 } }), "the other member is offline");
    });

    it("the header does not say \"only you so far\" for a long-running node either", () => {
      for (const transport of [undefined, dht(false), dht(true)]) {
        for (const peers of [[SELF], [SELF, OFFLINE], [SELF, ONLINE], []]) {
          assert.doesNotMatch(othersOnlineText({ peers, transport, node: { uptime_sec: 3600 } }), /only you so far/);
        }
      }
    });
  });
});

// The snapshot's own row; other members: known but offline (address cache), and an online one.
const SELF = { self: true, online: true };
const OFFLINE = { self: false, online: false };
const ONLINE = { self: false, online: true };

// The snapshot block about the address directory: `joined` — the node has joined the shared directory.
const dht = (joined, nodes = 0) => ({ dht: { joined, nodes } });
