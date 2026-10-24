/* `status-line.ts`: the header status line, "N online" and the identity strip. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { createConn } from "./conn.ts";
import { identityStrip, othersOnlineText, statusLine } from "./status-line.ts";
import { membersReply } from "../testing/members-fixtures.mjs";

describe("statusLine", () => {
  describe("node not visible", () => {
    it("awaitingToken — \"A new page link is needed\", takes priority over \"stopped\" and \"not responding\"", () => {
      const conn = createConn();
      conn.awaitingToken = true;
      const want = { level: "grey", text: "A new page link is needed" };
      assert.deepEqual(statusLine(node("running"), conn), want);
      conn.stopped = true;
      assert.deepEqual(statusLine(node("running"), conn), want);
      assert.deepEqual(statusLine(null, conn), want);
    });

    it("state === null — grey \"Mesh Town is not responding\"", () => {
      assert.deepEqual(statusLine(null, createConn()), { level: "grey", text: "Mesh Town is not responding" });
    });

    it("disconnected strip with a live state — also \"Mesh Town is not responding\"", () => {
      const conn = createConn();
      conn.strip = { kind: "disconnected", text: "Mesh Town is not responding" };
      assert.deepEqual(statusLine(node("running"), conn), { level: "grey", text: "Mesh Town is not responding" });
    });

    it("stopped — grey \"<program name> is stopped\"", () => {
      const conn = createConn();
      conn.stopped = true;
      assert.deepEqual(statusLine(node("running"), conn), { level: "grey", text: NET_NAME + " is stopped" });
    });
  });

  describe("node changes state", () => {
    it("starting — \"Mesh Town is starting…\"", () => {
      assert.deepEqual(statusLine(node("starting"), createConn()), { level: "grey", text: NET_NAME + " is starting…" });
    });

    it("stopping — \"Mesh Town is stopping…\"", () => {
      assert.deepEqual(statusLine(node("stopping"), createConn()), { level: "grey", text: NET_NAME + " is stopping…" });
    });
  });

  describe("no network", () => {
    it("grey \"You haven't joined a Mesh Town network yet\"", () => {
      const got = statusLine(node("running", { network: null }), createConn());
      assert.deepEqual(got, { level: "grey", text: "You haven't joined a Mesh Town network yet" });
    });

    it("the node is looking for the network by invite — \"Looking for the network \"…\"…\", as on Home", () => {
      const invite = { state: "waiting", message: "", label: "home", expires_unix: 1 };
      const got = statusLine(node("running", { network: null, node: { status: "running", invite_join: invite } }), createConn());
      assert.deepEqual(got, { level: "grey", text: "Looking for the network \"home\"…" });
    });
  });

  describe("network present", () => {
    it("direct — green dot, \"Sites open directly…\", label and member count", () => {
      const got = statusLine(node("running", { route: { state: "direct" }, peers: online(2) }), createConn());
      assert.equal(got.level, "green");
      assert.equal(got.text, "Sites open directly · family · 2 members online");
    });

    it("exit with an active exit — \"Sites open via …\", the name without an id", () => {
      const route = { state: "exit", exits: [{ id: "bbbb2222", name: "peer-one", active: true }] };
      const got = statusLine(node("running", { route, peers: online(0), transport: dht(true) }), createConn());
      assert.equal(got.level, "green");
      assert.match(got.text, /^Sites open via peer-one · family · /);
      assert.ok(!got.text.includes("bbbb2222"), got.text);
      assert.match(got.text, / · the other member is offline$/);
    });

    it("unavailable with one exit — red dot, \"is offline\"", () => {
      const route = { state: "unavailable", exits: [{ id: "bbbb2222", name: "peer-one" }] };
      const got = statusLine(node("running", { route }), createConn());
      assert.equal(got.level, "red");
      assert.match(got.text, /^peer-one is offline/);
      assert.ok(!got.text.includes("bbbb2222"), got.text);
    });

    it("unavailable with two exits — red dot, \"the chosen members are offline\"", () => {
      const route = { state: "unavailable", exits: [{ id: "a1" }, { id: "b2" }] };
      const got = statusLine(node("running", { route }), createConn());
      assert.equal(got.level, "red");
      assert.match(got.text, /offline/);
    });

    describe("members tail: three network states, direct route", () => {
      const HEAD = "Sites open directly · family · ";

      it("only the own row, directory connected — \"invite members\", green dot", () => {
        const got = statusLine(node("running", { peers: [SELF], transport: dht(true) }), createConn());
        assert.equal(got.level, "green");
        assert.equal(got.text, HEAD + "invite members");
      });

      it("only the own row, directory not connected — \"loading network data…\", not \"invite\" and not \"only you so far\"", () => {
        const got = statusLine(node("running", { peers: [SELF], transport: dht(false) }), createConn());
        assert.equal(got.text, HEAD + "loading network data…");
      });

      it("snapshot without transport — also \"loading network data…\"", () => {
        const got = statusLine(node("running", { peers: [SELF] }), createConn());
        assert.equal(got.text, HEAD + "loading network data…");
      });

      it("one other member offline, directory connected — \"the other member is offline\"", () => {
        const got = statusLine(node("running", { peers: [SELF, OFFLINE], transport: dht(true) }), createConn());
        assert.equal(got.text, HEAD + "the other member is offline");
      });

      it("two other members offline, directory connected — \"all other members are offline\"", () => {
        const got = statusLine(node("running", { peers: [SELF, OFFLINE, OFFLINE], transport: dht(true) }), createConn());
        assert.equal(got.text, HEAD + "all other members are offline");
      });

      it("other members offline, directory not connected — \"loading network data…\"", () => {
        const got = statusLine(node("running", { peers: [SELF, OFFLINE, OFFLINE], transport: dht(false) }), createConn());
        assert.equal(got.text, HEAD + "loading network data…");
      });

      it("another member online, directory not connected — \"1 member online\": loading does not override connectivity", () => {
        const got = statusLine(node("running", { peers: [SELF, ONLINE], transport: dht(false) }), createConn());
        assert.equal(got.text, HEAD + "1 member online");
      });
    });

    it("label null or empty — no empty network name and no \"null\"", () => {
      for (const label of [null, "", "  "]) {
        const got = statusLine(node("running", { network: { label } }), createConn());
        assert.ok(!got.text.includes("null"), got.text);
        assert.ok(!got.text.includes("undefined"), got.text);
        assert.ok(!got.text.includes("network \""), got.text);
      }
    });
  });
});

// This computer was removed from the member list: the node still reports the network, the header goes grey.
describe("statusLine of a computer removed from the member list", () => {
  const REMOVED = () => membersReply({ me_removed: true });

  it("grey_line_when_me_removed", () => {
    const got = statusLine(node("running"), createConn(), REMOVED());
    assert.deepEqual(got, { level: "grey", text: "Removed from the member list \"family\" \u2014 ask a member for an invite" });
  });

  it("ban_line_wins_over_removed: a ban stays red", () => {
    const banned = node("running", { members: { me_excluded: true } });
    const got = statusLine(banned, createConn(), REMOVED());
    assert.equal(got.level, "red");
    assert.match(got.text, /^Banned from the network "family"/);
  });

  describe("no_line_without_members", () => {
    it("members not given: the usual line of the network", () => {
      const got = statusLine(node("running"), createConn());
      assert.equal(got.level, "green");
      assert.match(got.text, /^Sites open directly/);
    });

    it("members null: the usual line of the network", () => {
      assert.match(statusLine(node("running"), createConn(), null).text, /^Sites open directly/);
    });

    it("members of a computer that is still listed: the usual line of the network", () => {
      const got = statusLine(node("running"), createConn(), membersReply());
      assert.equal(got.level, "green");
      assert.doesNotMatch(got.text, /Removed/);
    });
  });

  it("without a network the header says it is not joined", () => {
    const got = statusLine(node("running", { network: null }), createConn(), REMOVED());
    assert.deepEqual(got, { level: "grey", text: "You haven't joined a Mesh Town network yet" });
  });

  it("a node that does not answer is still reported as such", () => {
    assert.deepEqual(statusLine(null, createConn(), REMOVED()), { level: "grey", text: "Mesh Town is not responding" });
  });
});

// `othersOnlineText(state)` takes the snapshot (`peers` + `transport`): three network phases — one wording for the header and Home.
describe("othersOnlineText", () => {
  // `peers` are functions: SELF/OFFLINE/ONLINE are declared at the bottom of the file, while `describe` reads the table when the module loads.
  const rows = [
    { others: "no other rows (only the own one)", peers: () => [SELF], noTransport: "loading network data…", notJoined: "loading network data…", joined: "invite members" },
    { others: "one other member offline", peers: () => [SELF, OFFLINE], noTransport: "loading network data…", notJoined: "loading network data…", joined: "the other member is offline" },
    { others: "two other members offline", peers: () => [SELF, OFFLINE, OFFLINE], noTransport: "loading network data…", notJoined: "loading network data…", joined: "all other members are offline" },
    { others: "one other member online", peers: () => [SELF, ONLINE], noTransport: "1 member online", notJoined: "1 member online", joined: "1 member online" },
  ];

  for (const row of rows) {
    describe(row.others, () => {
      it(`no transport in the snapshot — "${row.noTransport}"`, () => {
        assert.equal(othersOnlineText({ peers: row.peers() }), row.noTransport);
      });

      it(`directory not connected (joined: false) — "${row.notJoined}"`, () => {
        assert.equal(othersOnlineText({ peers: row.peers(), transport: dht(false) }), row.notJoined);
      });

      it(`directory connected (joined: true) — "${row.joined}"`, () => {
        assert.equal(othersOnlineText({ peers: row.peers(), transport: dht(true) }), row.joined);
      });
    });
  }

  describe("other members online — \"N members online\", directory does not matter", () => {
    it("pluralizes the count (2/5/21)", () => {
      assert.equal(othersOnlineText({ peers: online(2) }), "2 members online");
      assert.equal(othersOnlineText({ peers: online(5) }), "5 members online");
      assert.equal(othersOnlineText({ peers: online(21) }), "21 members online");
    });

    it("counts only others who are online — not itself, not offline; the plain \"N members online\", no word \"more\"", () => {
      assert.equal(othersOnlineText({ peers: online(1), transport: dht(true) }), "1 member online");
    });

    it("one online and one offline — only the online one is counted", () => {
      assert.equal(othersOnlineText({ peers: [SELF, OFFLINE, ONLINE], transport: dht(true) }), "1 member online");
    });
  });

  describe("snapshot edge cases", () => {
    it("peers not set, directory connected — \"invite members\": no other rows", () => {
      assert.equal(othersOnlineText({ transport: dht(true) }), "invite members");
    });

    it("peers empty, directory connected — the same \"invite members\"", () => {
      assert.equal(othersOnlineText({ peers: [], transport: dht(true) }), "invite members");
    });

    it("peers null, directory connected — the same \"invite members\"", () => {
      assert.equal(othersOnlineText({ peers: null, transport: dht(true) }), "invite members");
    });

    it("peers empty, no directory — \"loading network data…\"", () => {
      assert.equal(othersOnlineText({ peers: [] }), "loading network data…");
    });

    it("an offline member without the self field is also another member: \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [SELF, { online: false }], transport: dht(true) }), "the other member is offline");
    });

    it("only other members offline, no own row — \"the other member is offline\"", () => {
      assert.equal(othersOnlineText({ peers: [{ online: false }], transport: dht(true) }), "the other member is offline");
    });

    it("the header no longer says \"only you so far\" in any state", () => {
      for (const peers of [[SELF], [SELF, OFFLINE], [SELF, ONLINE], []]) {
        for (const transport of [undefined, dht(false), dht(true)]) {
          assert.doesNotMatch(othersOnlineText({ peers, transport }), /only you so far/);
        }
      }
    });
  });
});

describe("identityStrip", () => {
  const CONSEQUENCE = "After a restart, other members will see this computer as a new member.";
  const CONSEQUENCE_HIM = CONSEQUENCE;

  it("identity is not temporary — null", () => {
    assert.equal(identityStrip(node("running", { node: { status: "running", identity: "persistent" } }), null), null);
    assert.equal(identityStrip(null, null), null);
  });

  it("declined without a file — fallback text and the declined class", () => {
    const got = identityStrip(temporary("declined"), null);
    assert.equal(got.cls, "identity-strip-declined");
    assert.equal(got.text, NET_NAME + " was started in a mode that doesn't remember this computer. " + CONSEQUENCE_HIM);
  });

  it("declined with file.message — the reason with a capital letter instead of the fallback text", () => {
    const got = identityStrip(temporary("declined"), { message: "the --ephemeral flag" });
    assert.equal(got.text, "The --ephemeral flag. " + CONSEQUENCE);
  });

  it("file_preserved — the note about the preserved file, the network name, the reason", () => {
    const got = identityStrip(temporary("file_preserved"), { message: "can't be rewritten" });
    assert.equal(got.cls, "identity-strip-preserved");
    assert.match(got.text, /^The settings file couldn't be updated, and the node left it as is/);
    assert.match(got.text, /access to your Mesh Town network\. /);
    assert.match(got.text, /Can't be rewritten\. After a restart/);
  });

  it("file_preserved without a reason — the note and the consequence", () => {
    const got = identityStrip(temporary("file_preserved"), null);
    assert.ok(got.text.endsWith("Mesh Town network. " + CONSEQUENCE), got.text);
  });

  it("any other reason — problem; field_errors joined with \"; \"", () => {
    const file = { message: "broken config", field_errors: [{ path: "socks.port", message: "not a number" }, { path: "a", message: "b" }] };
    const got = identityStrip(temporary("bad_file"), file);
    assert.equal(got.cls, "identity-strip-problem");
    assert.equal(got.text, "Broken config; Proxy port (SOCKS5 and HTTP) (socks.port): not a number; a: b. " + CONSEQUENCE);
  });

  it("any other reason without a file — fallback text about the settings file", () => {
    const got = identityStrip(temporary("bad_file"), null);
    assert.equal(got.text, NET_NAME + " couldn't remember this computer: there is an error in the settings file. " + CONSEQUENCE_HIM);
  });

  it("only field_errors, no message — the reason comes from them", () => {
    const got = identityStrip(temporary("bad_file"), { field_errors: [{ path: "x", message: "y" }] });
    assert.equal(got.text, "X: y. " + CONSEQUENCE);
  });
});

function node(status, over = {}) {
  return {
    node: { status },
    network: { label: "family" },
    route: { state: "direct" },
    peers: [],
    ...over,
  };
}

function temporary(reason) {
  return { node: { status: "running", identity: "temporary", identity_reason: reason } };
}

// The snapshot's own row: "just me" — when `peers` has no other rows.
const SELF = { self: true, online: true };
// Other members: known but offline (a row from the address cache), and an online one.
const OFFLINE = { self: false, online: false };
const ONLINE = { self: false, online: true };

// The snapshot block about the address directory: `joined` — the node has joined the shared directory.
const dht = (joined, nodes = 0) => ({ dht: { joined, nodes } });

// The node itself and one offline member are not counted; `n` other members online.
function online(n) {
  const peers = [{ self: true, online: true }, { self: false, online: false }];
  for (let i = 0; i < n; i++) peers.push({ self: false, online: true });
  return peers;
}
