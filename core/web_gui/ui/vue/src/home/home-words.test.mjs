/* The words of "Home" (`home-words.ts`): the member counter, the network status, the hint
 * under the "Allow others" switch. The cases of the classic `07-home-words.test.mjs` are carried over. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { exitServerHint, membersCountText, netPhase, netStatusText, onlineCountText } from "./home-words.ts";

describe("onlineCountText", () => {
  describe("all online", () => {
    it("one, two, more", () => {
      assert.equal(onlineCountText(1, 1), "online");
      assert.equal(onlineCountText(2, 2), "both online");
      assert.equal(onlineCountText(3, 3), "all online");
    });
  });

  describe("not all", () => {
    it("some — \"online: N\"", () => assert.equal(onlineCountText(3, 2), "2 online"));
    it("nobody — \"none online\"", () => assert.equal(onlineCountText(2, 0), "none online"));
  });
});

describe("membersCountText", () => {
  it("a network with a label — the string starts with \"Network \"family\" · \"", () => {
    const s = state({ peers: [] });
    s.network = { ...s.network, label: "family" };
    assert.equal(membersCountText(s), "Only you in \"family\" so far. Invite the people you trust.");
  });

  it("no label — a string without \"Network …\"", () => {
    const s = state({ peers: [] });
    s.network = { ...s.network, label: null };
    // Without a label the alone text stays the old one.
    assert.equal(membersCountText(s), "Only you for now");
  });

  it("no network — null", () => {
    assert.equal(membersCountText(state({ network: null })), null);
  });

  // `membersCountText` takes the whole `state` snapshot: "no network → null" can't be derived from `peers` alone.
  it("peers is empty — \"Only you in …\"", () => {
    assert.equal(membersCountText(state({ peers: [] })), "Only you in \"home\" so far. Invite the people you trust.");
  });

  it("one peer (only your own row) — \"Only you in …\"", () => {
    assert.equal(membersCountText(state({ peers: [self()] })), "Only you in \"home\" so far. Invite the people you trust.");
  });

  it("others are counted without you: three others, two online — \"you and 3 others, 2 online\"", () => {
    const peers = [self(), online(), online(), offline()];
    assert.equal(membersCountText(state({ peers })), "Network \"home\" · you and 3 others, 2 online");
  });

  it("two others, both online — \"you and 2 others, both online\"", () => {
    assert.equal(membersCountText(state({ peers: [self(), online(), online()] })), "Network \"home\" · you and 2 others, both online");
  });

  it("three others, all online — \"you and 3 others, all online\"", () => {
    assert.equal(membersCountText(state({ peers: [self(), online(), online(), online()] })), "Network \"home\" · you and 3 others, all online");
  });

  it("one other, online — \"you and 1 other, online\"", () => {
    assert.equal(membersCountText(state({ peers: [self(), online()] })), "Network \"home\" · you and 1 other, online");
  });

  it("two others, none online — your own online row is not counted", () => {
    assert.equal(membersCountText(state({ peers: [self(), offline(), offline()] })), "Network \"home\" · you and 2 others, none online");
  });
});

// The three network phases: the check order is another member online → address directory not connected → others exist → no others.
describe("netPhase", () => {
  const rows = [
    { others: "no other rows (only your own)", peers: [self()], noTransport: "loading", notJoined: "loading", joined: "alone" },
    { others: "one other member offline", peers: [self(), offline()], noTransport: "loading", notJoined: "loading", joined: "offline" },
    { others: "two other members offline", peers: [self(), offline(), offline()], noTransport: "loading", notJoined: "loading", joined: "offline" },
    { others: "one other member online", peers: [self(), online()], noTransport: "online", notJoined: "online", joined: "online" },
  ];

  for (const row of rows) {
    describe(row.others, () => {
      it(`no transport in the snapshot — ${row.noTransport}`, () => {
        assert.equal(netPhase(state({ peers: row.peers })), row.noTransport);
      });

      it(`address directory not connected (joined: false) — ${row.notJoined}`, () => {
        assert.equal(netPhase(state({ peers: row.peers, transport: dht(false) })), row.notJoined);
      });

      it(`address directory connected (joined: true) — ${row.joined}`, () => {
        assert.equal(netPhase(state({ peers: row.peers, transport: dht(true, 5) })), row.joined);
      });
    });
  }

  describe("edge cases", () => {
    it("transport exists but has no dht — directory not connected, loading", () => {
      assert.equal(netPhase(state({ peers: [self(), offline()], transport: {} })), "loading");
    });

    it("another member online and one offline — online: one online is enough", () => {
      assert.equal(netPhase(state({ peers: [self(), offline(), online()], transport: dht(true) })), "online");
    });

    it("peers is empty, directory connected — alone", () => {
      assert.equal(netPhase(state({ peers: [], transport: dht(true) })), "alone");
    });

    it("peers is not set, directory connected — alone", () => {
      assert.equal(netPhase(state({ peers: undefined, transport: dht(true) })), "alone");
    });

    it("another member without a self field counts as other: offline — offline", () => {
      assert.equal(netPhase(state({ peers: [self(), { online: false }], transport: dht(true) })), "offline");
    });

    it("your own row being offline doesn't make \"another member online\": no others — alone", () => {
      assert.equal(netPhase(state({ peers: [{ self: true, online: false }], transport: dht(true) })), "alone");
    });
  });
});

describe("netStatusText", () => {
  it("no network — null", () => assert.equal(netStatusText(state({ network: null, peers: [] })), null));

  describe("loading network data (address directory not connected)", () => {
    it("only your own row in the snapshot — \"Loading network data…\"", () => {
      assert.equal(netStatusText(state({ peers: [self()], transport: dht(false) })), LOADING);
    });

    it("peers is empty — the same \"Loading network data…\"", () => {
      assert.equal(netStatusText(state({ peers: [], transport: dht(false) })), LOADING);
    });

    it("a known other member offline — loading too, not \"offline\": without the directory's answer we don't claim that", () => {
      assert.equal(netStatusText(state({ peers: [self(), offline()], transport: dht(false) })), LOADING);
    });

    it("no transport — counts as \"not connected\": loading", () => {
      assert.equal(netStatusText(state({ peers: [self()] })), LOADING);
      assert.equal(netStatusText(state({ peers: [self(), offline()] })), LOADING);
    });

    it("no words about the invite or about \"searching\"", () => {
      const text = netStatusText(state({ peers: [self()], transport: dht(false) }));
      assert.doesNotMatch(text, /Invite a member|Looking for members|connecting/);
    });
  });

  describe("only you in the network, address directory connected (alone)", () => {
    it("your own row, empty directory (nodes 0) — a hint about the invite", () => {
      assert.equal(netStatusText(state({ peers: [self()], transport: dht(true, 0) })), ALONE_INVITE);
    });

    it("900000 nodes in the directory — the same text, the number doesn't get into it", () => {
      const text = netStatusText(state({ peers: [self()], transport: dht(true, 900000) }));
      assert.equal(text, ALONE_INVITE);
      assert.doesNotMatch(text, /900000/, "the size of the shared DHT is not shown in the text");
    });

    it("peers is empty — the same as with your own row", () => {
      assert.equal(netStatusText(state({ peers: [], transport: dht(true, 0) })), ALONE_INVITE);
    });

    it("peers is not set — the same as with your own row", () => {
      assert.equal(netStatusText(state({ peers: undefined, transport: dht(true, 0) })), ALONE_INVITE);
    });

    it("no words about \"offline\"", () => {
      assert.doesNotMatch(netStatusText(state({ peers: [self()], transport: dht(true, 0) })), /offline/);
    });
  });

  describe("others are known, all offline, address directory connected (offline)", () => {
    it("one other member — \"The other member is offline — looking for them through the address directory.\"", () => {
      const text = netStatusText(state({ peers: [self(), offline()], transport: dht(true, 0) }));
      assert.equal(text, "The other member is offline — looking for them through the address directory.");
    });

    it("two other members — \"All other members are offline — looking for them through the address directory.\"", () => {
      const text = netStatusText(state({ peers: [self(), offline(), offline()], transport: dht(true, 0) }));
      assert.equal(text, "All other members are offline — looking for them through the address directory.");
    });

    it("three other members — also \"All other…\" (n ≥ 2)", () => {
      const text = netStatusText(state({ peers: [self(), offline(), offline(), offline()], transport: dht(true, 0) }));
      assert.match(text, /^All other members are offline — /);
    });

    it("900000 nodes in the directory — the number doesn't get into the text; no hint about the invite", () => {
      const text = netStatusText(state({ peers: [self(), offline()], transport: dht(true, 900000) }));
      assert.doesNotMatch(text, /900000|Invite a member/);
    });

    it("no old wording: we no longer say \"Other members are offline for now…\"", () => {
      const text = netStatusText(state({ peers: [self(), offline()], transport: dht(true, 0) }));
      assert.doesNotMatch(text, /Other members are offline for now|searching for them/);
    });
  });

  describe("another member online (online)", () => {
    it("another member online, directory not connected — null", () => {
      assert.equal(netStatusText(state({ peers: [self(), online()], transport: dht(false) })), null);
    });

    it("another member online, directory connected — null: no hint needed", () => {
      assert.equal(netStatusText(state({ peers: [self(), online()], transport: dht(true, 0) })), null);
    });

    it("another member online, no transport — null", () => {
      assert.equal(netStatusText(state({ peers: [self(), online()] })), null);
    });
  });
});

describe("exitServerHint", () => {
  it("on — members of the network can exit via your internet, sites will see your address", () => {
    assert.equal(
      exitServerHint(true, "family"),
      "On: members of the network \"family\" can open sites through your internet connection, and sites will see your address."
    );
  });

  it("off — \"nobody opens sites through this computer\"", () => {
    assert.equal(exitServerHint(false, "family"), "Off: no one can open sites through this computer.");
  });

  it("no label — no empty quotes and no \"null\"", () => {
    const text = exitServerHint(true, null);
    assert.equal(text, "On: members of the network can open sites through your internet connection, and sites will see your address.");
  });
});

// The texts are verbatim, not from `common/words.ts`: the test checks the words, not a reference to the constant.
const ALONE_INVITE = "Click \"Invite a member\" and send the invite — the new member will appear here as soon as they connect.";
const LOADING = "Loading network data…";

function self() {
  return { self: true, online: true };
}

// A known other member who is offline right now (a row from the address cache or a member who left).
function offline() {
  return { self: false, online: false };
}

// Another member who is online.
function online() {
  return { self: false, online: true };
}

const dht = (joined, nodes = 0) => ({ dht: { joined, nodes } });

function state(over = {}) {
  return { network: { label: "home", id_hex: "n" }, peers: [], ...over };
}
