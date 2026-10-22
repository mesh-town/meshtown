/* "The address directory is unavailable": the node has been running for a long time (≥ 60 s), itself
 * reported `transport.dht.joined === false`, and none of the others is online — Home (`netStatusText`) and the header
 * (`othersOnlineText`) say so and call to invite a member: the invite has your address, the connection goes
 * directly, no directory needed. The words are written verbatim, not by importing constants: the test checks the page text,
 * not the code matching itself. The phase and the 60 s boundary are in `net-phase-grace.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createConn } from "../shell/conn.ts";
import { othersOnlineText, statusLine } from "../shell/status-line.ts";
import { membersCountText, netStatusText } from "./home-words.ts";

const HEADER_TAIL = "members can't find you automatically";
const ALONE =
  "Mesh Town can't reach the address directory that helps members find each other. Invite members anyway: the invite contains your address, so they can connect without it.";
const ONE_OFFLINE =
  "The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
const ALL_OFFLINE =
  "All other members are offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";

describe("address directory unavailable: node running for a long time, joined: false, no others online", () => {
  describe("Home (netStatusText)", () => {
    describe("no other rows", () => {
      it("calls to invite a member: the invite has your address", () => {
        assert.equal(setup({ others: 0 }).statusText(), ALONE);
      });
    });

    describe("one other offline", () => {
      it("\"The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite…\"", () => {
        assert.equal(setup({ others: 1 }).statusText(), ONE_OFFLINE);
      });
    });

    describe("two others offline", () => {
      it("\"All other members are offline, and the address directory that helps members find each other is unavailable. Send an invite…\"", () => {
        assert.equal(setup({ others: 2 }).statusText(), ALL_OFFLINE);
      });
    });

    describe("three others offline", () => {
      it("the same words as for two: \"All other members are offline, and the directory…\"", () => {
        assert.equal(setup({ others: 3 }).statusText(), ALL_OFFLINE);
      });
    });

    describe("words from the project dictionary", () => {
      for (const others of [0, 1, 2, 3]) {
        it(`${others} others: "address directory" is present, no "DHT", "relay"`, () => {
          const text = setup({ others }).statusText();
          assert.match(text, /address directory/i);
          assert.doesNotMatch(text, /DHT|relay|Loading|searching via the shared address directory/i);
        });
      }
    });

    describe("the directory record existed before (the node dropped mid-run)", () => {
      it("the same words as for \"never connected\": no need to tell them apart", () => {
        const state = setup({ others: 1 }).state;
        const dropped = { ...state, transport: { ...state.transport, record: { published: true, ttl_sec: 3600 } } };
        assert.equal(netStatusText(dropped), ONE_OFFLINE);
      });
    });

    describe("member counter on Home", () => {
      it("those offline are still counted: \"you and 2 others, none online\"", () => {
        assert.equal(membersCountText(setup({ others: 2 }).state), "Network \"family\" · you and 2 others, none online");
      });
    });

    describe("no network (no network in the snapshot)", () => {
      it("no status (null)", () => {
        const noNetwork = { ...setup({ others: 1 }).state, network: null };
        assert.equal(netStatusText(noNetwork), null);
      });
    });
  });

  describe("header (othersOnlineText)", () => {
    const WANT = { 0: HEADER_TAIL, 1: "the other member is offline", 2: "all other members are offline", 3: "all other members are offline" };
    for (const others of [0, 1, 2, 3]) {
      describe(`${others} others offline`, () => {
        it(`tail — "${WANT[others]}"`, () => {
          assert.equal(setup({ others }).headerTail(), WANT[others]);
        });

        it("the status line has the same tail", () => {
          const text = setup({ others }).statusLineText();
          assert.ok(text.endsWith(" · family · " + WANT[others]), text);
        });
      });
    }
  });

  describe("another member online", () => {
      it("Home shows no status (null): the connection with a member matters more than the directory", () => {
      assert.equal(setup({ others: 2, online: 1 }).statusText(), null);
    });

    it("the header counts those online: \"1 member online\"", () => {
      assert.equal(setup({ others: 2, online: 1 }).headerTail(), "1 member online");
    });
  });
});

describe("address directory connected (joined: true), node running for a long time — the usual words", () => {
  it("no others: Click \"Invite a member\" and send the invite…", () => {
    const want = "Click \"Invite a member\" and send the invite — the new member will appear here as soon as they connect.";
    assert.equal(setup({ others: 0, joined: true }).statusText(), want);
  });

  it("one other offline: \"… — looking for them through the address directory.\"", () => {
    const want = "The other member is offline — looking for them through the address directory.";
    assert.equal(setup({ others: 1, joined: true }).statusText(), want);
  });

  it("two others offline: \"All other members are offline — looking for them through the address directory.\"", () => {
    const want = "All other members are offline — looking for them through the address directory.";
    assert.equal(setup({ others: 2, joined: true }).statusText(), want);
  });

  it("header: \"the other member is offline\", nothing about the directory", () => {
    assert.equal(setup({ others: 1, joined: true }).headerTail(), "the other member is offline");
  });
});

describe("node started recently (30 s), no directory — still \"loading\"", () => {
  it("Home: \"Loading network data…\"", () => {
    assert.equal(setup({ others: 1, uptime: 30 }).statusText(), "Loading network data…");
  });

  it("header: \"loading network data…\"", () => {
    assert.equal(setup({ others: 1, uptime: 30 }).headerTail(), "loading network data…");
  });
});

// Snapshot of a long-running node (120 s by default) with the directory `joined` (false by default), its own row, `others` other rows
// (`online` of them online, the rest not) — and the page words from it.
function setup({ others, online = 0, joined = false, uptime = 120 }) {
  const peers = [{ self: true, online: true }];
  for (let i = 0; i < others; i++) peers.push({ self: false, online: i < online });
  const state = {
    node: { status: "running", uptime_sec: uptime },
    network: { label: "family", id_hex: "n" },
    route: { state: "direct" },
    transport: { dht: { joined, nodes: 0, last_publish_sec: null } },
    peers,
  };
  return {
    state,
    statusText: () => netStatusText(state),
    headerTail: () => othersOnlineText(state),
    statusLineText: () => statusLine(state, createConn()).text,
  };
}
