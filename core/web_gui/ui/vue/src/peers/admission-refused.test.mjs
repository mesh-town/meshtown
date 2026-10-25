/* The names of the "turned this computer down" plate and the server flag that decides whether it stands. */

import { describe, expect, it } from "vitest";
import { memberRow, membersReply, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";
import { refusedNow, refusingNames } from "./admission-refused.ts";

const them = { side: "them", reason: "no", at_unix: 1 };
const me = { side: "me", reason: "no", at_unix: 1 };
const off = (over = {}) => peerOneRow({ online: false, last_refusal: them, ...over });

describe("refusingNames", () => {
  it("the members that refused, by name", () => {
    const reply = membersReply({ rows: [selfRow(), off(), peerTwoRow({ name: "peer-two", last_refusal: them })] });
    expect(refusingNames(reply)).toEqual(["peer-one", "peer-two"]);
  });

  it("null and undefined: no names", () => {
    expect(refusingNames(null)).toEqual([]);
    expect(refusingNames(undefined)).toEqual([]);
  });

  it("the own row is not a name", () => {
    expect(refusingNames(membersReply({ rows: [selfRow({ last_refusal: them })] }))).toEqual([]);
  });

  it("an online neighbour does not change the names", () => {
    const reply = membersReply({ rows: [selfRow(), off(), memberRow({ id: "dddd4444", id_full: "D".repeat(64), online: true })] });
    expect(refusingNames(reply)).toEqual(["peer-one"]);
  });

  it("\"unlisted\" is not a refusal: the list is still reaching the members", () => {
    const unlisted = { side: "them", reason: "unlisted", at_unix: 1 };
    const reply = membersReply({ rows: [selfRow(), off({ last_refusal: unlisted }), peerTwoRow({ name: "peer-two", online: false, last_refusal: them })] });
    expect(refusingNames(reply)).toEqual(["peer-two"]);
  });

  it("a refusal on our side and no refusal: no names", () => {
    expect(refusingNames(membersReply({ rows: [selfRow(), off({ last_refusal: me })] }))).toEqual([]);
    expect(refusingNames(membersReply({ rows: [selfRow(), off({ last_refusal: null })] }))).toEqual([]);
  });
});

describe("refusedNow", () => {
  const reply = () => membersReply({ rows: [selfRow(), off()] });
  it("flag off: null even with a refusal in the list", () => {
    expect(refusedNow(false, reply())).toBeNull();
  });
  it("flag on and no list: empty names", () => {
    expect(refusedNow(true, null)).toEqual({ names: [] });
  });
  it("flag on and a refusal: the names", () => {
    expect(refusedNow(true, reply())).toEqual({ names: ["peer-one"] });
  });
});
