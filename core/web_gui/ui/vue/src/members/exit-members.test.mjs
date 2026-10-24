/* `exit-members.ts`: what the member list means for the exit you chose. All functions are pure, named exports of `./exit-members.ts`.
 *
 * Contract fixed by these tests:
 *  - `exitWarning(chosen, members, meExcluded)` looks at the chosen exits in the order of `chosen` and reports the first one whose row in
 *    `members` is excluded (`excluded.how === "left"` → kind "left", anything else → "banned") or blocked on this computer
 *    (`blocked_by_me`, or `denied_by_me === "voted_by_me"` — my own ban proposal blocks it locally → kind "blocked").
 *    The result is `{ id, kind, text, hint }`, `text` is "Your exit {name} {was banned from the network | left the network | is blocked on
 *    this computer} — sites won't open until you choose another exit" (no final period); the name is the `displayNames` one.
 *    `hint` is the short line above the exit list: "{name} {was banned from the network | left the network | is blocked on this
 *    computer}. Choose another exit below."
 *    `null` when nothing is wrong, when `meExcluded` is set (the whole computer is out then) or when `members` is null (no list).
 *  - `memberExits(candidates, members)` drops the excluded candidates and marks the blocked ones with `blocked: true`.
 *  - `exitMetaText` of a blocked candidate is "Blocked on this computer" (online or not).
 *  - `withExitWarning(line, warning)` turns a green or red header line into `{ level: "red", text, alert: true }`; a grey line stays.
 *  - `warnPanel(panel, warning)` turns a green or red state panel into `{ level: "red", main, sub: "", actions: ["pick-other", "direct"] }`;
 *    a grey panel stays. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { exitMetaText } from "../home/exit-list.ts";
import { exitWarning, memberExits, warnPanel, withExitWarning } from "./exit-members.ts";
import { PEER2_FULL, PEER_FULL } from "../testing/section.mjs";
import { excludedInfo, membersReply, peerOneRow, peerTwoRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";

const TAIL = " — sites won't open until you choose another exit";
const REMOVED_TEXT = "Your exit peer-one was banned from the network" + TAIL;
const REMOVED_HINT = "peer-one was banned from the network. Choose another exit below.";
const WARNING = { id: PEER_FULL, kind: "removed", text: REMOVED_TEXT, hint: REMOVED_HINT };

const reply = (...rows) => membersReply({ rows: [selfRow(), ...rows] });
const removedOne = (over = {}) => peerOneRow({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo(), ...over });
const leftOne = () => removedOne({ denied_by_me: "left", excluded: excludedInfo({ how: "left", voters: [] }) });
const blockedOne = () => peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" });
const proposedByMe = () => peerOneRow({ denied_by_me: "voted_by_me", vote: voteInfo() });
const candidate = (idFull, over = {}) => ({
  idFull, label: "member " + idFull.slice(0, 4), country: null, online: true, active: false, path: "direct", rttMs: 30, checked: false, ...over,
});

describe("exitWarning", () => {
  describe("the chosen exit was banned from the network", () => {
    it("reports kind removed with the sentence about the removal", () => {
      const warning = exitWarning([PEER_FULL], reply(removedOne()), false);
      assert.deepEqual(warning, WARNING);
    });
  });

  describe("the chosen exit left the network on its own", () => {
    it("reports kind left with the sentence about leaving", () => {
      const warning = exitWarning([PEER_FULL], reply(leftOne()), false);
      assert.deepEqual(warning, { id: PEER_FULL, kind: "left", text: "Your exit peer-one left the network" + TAIL, hint: "peer-one left the network. Choose another exit below." });
    });
  });

  describe("the chosen exit is blocked on this computer", () => {
    it("blocked_by_me: reports kind blocked with the sentence about the block", () => {
      const warning = exitWarning([PEER_FULL], reply(blockedOne()), false);
      assert.deepEqual(warning, { id: PEER_FULL, kind: "blocked", text: "Your exit peer-one is blocked on this computer" + TAIL, hint: "peer-one is blocked on this computer. Choose another exit below." });
    });

    it("my own ban proposal (voted_by_me) blocks it here too", () => {
      const warning = exitWarning([PEER_FULL], reply(proposedByMe()), false);
      assert.equal(warning.kind, "blocked");
      assert.equal(warning.id, PEER_FULL);
    });

    it("a removed member that is also blocked is reported as removed", () => {
      const warning = exitWarning([PEER_FULL], reply(removedOne({ blocked_by_me: true })), false);
      assert.equal(warning.kind, "removed");
    });
  });

  describe("the member has no name", () => {
    it("the warning uses the short id", () => {
      const nameless = peerTwoRow({ name: null, blocked_by_me: true, denied_by_me: "blocked" });
      const warning = exitWarning([PEER2_FULL], reply(nameless), false);
      assert.equal(warning.text, "Your exit cccc3333 is blocked on this computer" + TAIL);
    });
  });

  describe("nothing is wrong", () => {
    it.each([
      ["the exit is an ordinary member", reply(peerOneRow())],
      ["the exit has a ban vote of someone else", reply(peerOneRow({ vote: voteInfo({ proposed_by: PEER2_FULL, mine: false, voters: [PEER2_FULL] }) }))],
      ["the exit is not in the member list", reply(peerTwoRow())],
      ["the exit row is unlisted (not blocked)", reply(peerOneRow({ status: "unlisted", denied_by_me: "unlisted" }))],
    ])("%s: null", (_name, members) => {
      assert.equal(exitWarning([PEER_FULL], members, false), null);
    });

    it("no exit is chosen (the usual way): null, even when a member is removed", () => {
      assert.equal(exitWarning([], reply(removedOne(), peerTwoRow()), false), null);
    });

    it("another member is removed, not the chosen one: null", () => {
      const members = reply(peerOneRow(), peerTwoRow({ status: "excluded", denied_by_me: "excluded", excluded: excludedInfo() }));
      assert.equal(exitWarning([PEER_FULL], members, false), null);
    });
  });

  describe("this computer is out of the network", () => {
    it("meExcluded: null — the whole computer is out, a separate card says so", () => {
      assert.equal(exitWarning([PEER_FULL], reply(removedOne()), true), null);
    });
  });

  describe("there is no member list (an older node, no network)", () => {
    it("null", () => {
      assert.equal(exitWarning([PEER_FULL], null, false), null);
    });
  });

  describe("several exits are chosen", () => {
    it("the healthy first one does not hide the broken second one", () => {
      const members = reply(removedOne(), peerTwoRow({ name: "peer-two" }));
      assert.equal(exitWarning([PEER2_FULL, PEER_FULL], members, false).id, PEER_FULL);
    });

    it("both broken: the first of the chosen wins, not the first of the list", () => {
      const two = peerTwoRow({ name: "peer-two", blocked_by_me: true, denied_by_me: "blocked" });
      const members = reply(removedOne(), two);
      assert.equal(exitWarning([PEER2_FULL, PEER_FULL], members, false).id, PEER2_FULL);
      assert.equal(exitWarning([PEER_FULL, PEER2_FULL], members, false).id, PEER_FULL);
    });
  });
});

describe("memberExits", () => {
  const list = () => [candidate(PEER_FULL), candidate(PEER2_FULL), candidate("D".repeat(64))];

  describe("the member list is known", () => {
    it("an excluded candidate is taken out, the others keep their order", () => {
      const out = memberExits(list(), reply(removedOne(), peerTwoRow()));
      assert.deepEqual(out.map((c) => c.idFull), [PEER2_FULL, "D".repeat(64)]);
    });

    it("a candidate who left is taken out too", () => {
      const out = memberExits(list(), reply(leftOne(), peerTwoRow()));
      assert.equal(out.some((c) => c.idFull === PEER_FULL), false);
    });

    it("a blocked candidate stays in the list with blocked: true and keeps its fields", () => {
      const out = memberExits(list(), reply(blockedOne(), peerTwoRow()));
      const blocked = out.find((c) => c.idFull === PEER_FULL);
      assert.equal(blocked.blocked, true);
      assert.deepEqual({ ...blocked, blocked: undefined }, { ...candidate(PEER_FULL), blocked: undefined });
    });

    it("a candidate I proposed banning (voted_by_me) is blocked too", () => {
      const out = memberExits(list(), reply(proposedByMe(), peerTwoRow()));
      assert.equal(out.find((c) => c.idFull === PEER_FULL).blocked, true);
    });

    it("ordinary members and members the list does not know have no blocked mark", () => {
      const out = memberExits(list(), reply(peerOneRow()));
      assert.deepEqual(out.map((c) => !!c.blocked), [false, false, false]);
      assert.equal(out.length, 3);
    });
  });

  describe("there is no member list", () => {
    it("every candidate stays and none is blocked", () => {
      const out = memberExits(list(), null);
      assert.deepEqual(out.map((c) => c.idFull), list().map((c) => c.idFull));
      assert.equal(out.some((c) => c.blocked), false);
    });
  });

  it("an empty list stays empty", () => {
    assert.deepEqual(memberExits([], reply(removedOne())), []);
  });
});

describe("exitMetaText of a blocked exit", () => {
  const online = { online: true, path: "direct", rttMs: 30 };

  it("is \"Blocked on this computer\", whatever the connection is", () => {
    assert.equal(exitMetaText({ ...online, blocked: true }), "Blocked on this computer");
  });

  it("is the same for an exit that is offline", () => {
    assert.equal(exitMetaText({ online: false, path: null, rttMs: null, blocked: true }), "Blocked on this computer");
  });

  it("blocked: false changes nothing", () => {
    assert.equal(exitMetaText({ ...online, blocked: false }), exitMetaText(online));
  });
});

describe("withExitWarning", () => {
  const GREEN = { level: "green", text: "In the browser, sites open directly" };

  it("no warning: the line stays as it is", () => {
    assert.deepEqual(withExitWarning(GREEN, null), GREEN);
  });

  it("a green line becomes a red alert with the text of the warning", () => {
    assert.deepEqual(withExitWarning(GREEN, WARNING), { level: "red", text: REMOVED_TEXT, alert: true });
  });

  it("a red line (the exit is offline) is replaced as well: the ban is the more important news", () => {
    const red = { level: "red", text: "peer-one is offline" };
    assert.deepEqual(withExitWarning(red, WARNING), { level: "red", text: REMOVED_TEXT, alert: true });
  });

  it("a grey line stays: \"not responding\" and \"starting\" say more than the warning", () => {
    const grey = { level: "grey", text: "Mesh Town is not responding" };
    assert.deepEqual(withExitWarning(grey, WARNING), grey);
  });

  it("does not change the line it was given", () => {
    const line = { level: "green", text: "x" };
    withExitWarning(line, WARNING);
    assert.deepEqual(line, { level: "green", text: "x" });
  });
});

describe("warnPanel", () => {
  const GREEN = { level: "green", main: "In the browser, sites open directly", sub: "Some hint", actions: [] };

  it("no warning: the panel stays as it is", () => {
    assert.deepEqual(warnPanel(GREEN, null), GREEN);
  });

  it("a green panel becomes red: the warning text, no sub line, the two ways out", () => {
    assert.deepEqual(warnPanel(GREEN, WARNING), { level: "red", main: REMOVED_TEXT, sub: "", actions: ["pick-other", "direct"] });
  });

  it("a red panel (the exit is offline) is replaced as well", () => {
    const red = { level: "red", main: "peer-one is offline", sub: "", actions: ["pick-other", "direct"] };
    assert.deepEqual(warnPanel(red, WARNING), { level: "red", main: REMOVED_TEXT, sub: "", actions: ["pick-other", "direct"] });
  });

  it("a grey panel stays: the node is starting or not responding", () => {
    const grey = { level: "grey", main: "Starting…", sub: "", actions: [], busy: true };
    assert.deepEqual(warnPanel(grey, WARNING), grey);
  });
});
