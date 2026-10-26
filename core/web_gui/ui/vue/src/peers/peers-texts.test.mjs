/* Texts of the multi-network "Members" section: the tab labels, the tab head and the hint. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import {
  ADMISSION_REFUSED_NONE,
  PEERS_INVITE_COPIED,
  PEERS_INVITE_ONE,
  PEERS_NETS_HINT_KEY,
  PEERS_NETS_HINT_OK,
  PEERS_NEW_NET,
  PEERS_TAB_BROKEN_SUB,
  peersBrokenNet,
  peersTabBrokenTitle,
  PEERS_TAB_JOINING,
  PEERS_TAB_JOINING_SUB,
  peersInviteTo,
  peersNetLine,
  peersNetsHint,
  peersTabSub,
  peersTabTitle,
  peersWhoText,
  admissionRefusedAsk,
} from "./peers-texts.ts";

describe("the refused-admission plate without names", () => {
  it("ADMISSION_REFUSED_NONE names the network", () => {
    assert.equal(ADMISSION_REFUSED_NONE("family"), 'Members of "family" refuse to let it in, and no other member is online.');
  });
  it("the ask goes to an admin", () => {
    assert.ok(admissionRefusedAsk("family").startsWith('Ask an admin of "family" '));
  });
});

describe("peersWhoText", () => {
  describe("nobody else", () => {
    it("is \"just you\"", () => assert.equal(peersWhoText(0, 0), "just you"));
  });

  describe("one other member", () => {
    it("online — \"online\"", () => assert.equal(peersWhoText(1, 1), "you and 1 other, online"));
    it("offline — \"0 online\"", () => assert.equal(peersWhoText(0, 1), "you and 1 other, 0 online"));
  });

  describe("two others", () => {
    it("both online — \"both online\"", () => assert.equal(peersWhoText(2, 2), "you and 2 others, both online"));
    it("one online — \"1 online\"", () => assert.equal(peersWhoText(1, 2), "you and 2 others, 1 online"));
    it("none online — \"0 online\"", () => assert.equal(peersWhoText(0, 2), "you and 2 others, 0 online"));
  });

  describe("three others", () => {
    it("all online — \"all online\"", () => assert.equal(peersWhoText(3, 3), "you and 3 others, all online"));
    it("part online — the count", () => assert.equal(peersWhoText(2, 3), "you and 3 others, 2 online"));
  });
});

describe("peersTabSub", () => {
  it("connected — the online count", () => assert.equal(peersTabSub(2, false), "· 2 online"));
  it("zero online but connected — still a count", () => assert.equal(peersTabSub(0, false), "· 0 online"));
  it("no connection — the count is not shown", () => assert.equal(peersTabSub(2, true), "· no connection"));
});

describe("peersTabTitle", () => {
  it("connected — \"N of M members online\"", () => {
    assert.equal(peersTabTitle("friends", 2, 3, false), "friends — 2 of 3 members online");
  });

  it("no connection — only the state", () => {
    assert.equal(peersTabTitle("my devices", 0, 3, true), "my devices — no connection");
  });
});

describe("tab head", () => {
  it("the invite button names the network", () => assert.equal(peersInviteTo("my devices"), "Invite to \"my devices\""));

  it("the grey line joins the network and who is in it", () => {
    assert.equal(peersNetLine("my devices", "just you"), "Network \"my devices\" · just you");
  });
});

describe("peersNetsHint", () => {
  it("names the number of networks", () => {
    assert.equal(
      peersNetsHint(3),
      "You’re now in 3 networks. Each network has its own tab here. " +
        "Your name, \"Let members open sites\" and \"Leave network\" are now in each network’s tab.",
    );
  });
});

describe("peersNetsHint in the app", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("the app hint does not name Let members open sites", () => {
    vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android" });
    assert.equal(
      peersNetsHint(2, true),
      "You’re now in 2 networks. Each network has its own tab here. Your name and \"Leave network\" are now in each network’s tab.",
    );
  });
});

describe("the broken-network texts", () => {
  it("name the tab and the reason", () => {
    assert.equal(PEERS_TAB_BROKEN_SUB, "· can't start");
    assert.equal(peersTabBrokenTitle("work"), "work — can't start: its saved settings can't be read");
    assert.equal(
      peersBrokenNet(),
      "This network can't start: its saved settings can't be read, for example a network key from an older version. Ask an admin for a new invite to join it again.",
    );
  });
});

describe("fixed strings", () => {
  it("the invite button of a single network", () => assert.equal(PEERS_INVITE_ONE, "Invite a member"));

  it("the copied-invite line", () => {
    assert.equal(PEERS_INVITE_COPIED, "Invite copied. It works once, for 24 hours — send it to one person in a private message.");
  });

  it("a network being joined", () => {
    assert.equal(PEERS_TAB_JOINING, "Joining the network…");
    assert.equal(PEERS_TAB_JOINING_SUB, "· joining…");
    assert.equal(PEERS_NEW_NET, "New network");
  });

  it("the hint button and its storage key", () => {
    assert.equal(PEERS_NETS_HINT_OK, "OK");
    assert.equal(PEERS_NETS_HINT_KEY, "meshtown.netsHintSeen");
  });
});
