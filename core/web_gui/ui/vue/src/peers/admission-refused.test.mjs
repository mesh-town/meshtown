/* The rule of the "turned this computer down" plate: refusals "them", nobody else online, no own card of the computer. */

import { describe, expect, it } from "vitest";
import { memberRow, membersReply, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";
import { admissionRefused } from "./admission-refused.ts";

const them = { side: "them", reason: "no", at_unix: 1 };
const me = { side: "me", reason: "no", at_unix: 1 };
const off = (over = {}) => peerOneRow({ online: false, last_refusal: them, ...over });

describe("admissionRefused", () => {
  it("a refusal by them and nobody else online: yes, with the names", () => {
    const reply = membersReply({ rows: [selfRow(), off(), peerTwoRow({ name: "peer-two", last_refusal: them })] });
    expect(admissionRefused(reply)).toEqual({ names: ["peer-one", "peer-two"] });
  });

  it("a refusal \"unlisted\" is not a refusal: the list is still reaching the members", () => {
    const unlisted = { side: "them", reason: "unlisted", at_unix: 1 };
    expect(admissionRefused(membersReply({ rows: [selfRow(), off({ last_refusal: unlisted })] }))).toBeNull();
    const mixed = membersReply({ rows: [selfRow(), off({ last_refusal: unlisted }), peerTwoRow({ name: "peer-two", online: false, last_refusal: them })] });
    expect(admissionRefused(mixed)).toEqual({ names: ["peer-two"] });
  });

  it("somebody else is online: no", () => {
    const reply = membersReply({ rows: [selfRow(), off(), memberRow({ id: "dddd4444", id_full: "D".repeat(64), online: true })] });
    expect(admissionRefused(reply)).toBeNull();
  });

  it("a refusal on our side: no", () => {
    expect(admissionRefused(membersReply({ rows: [selfRow(), off({ last_refusal: me })] }))).toBeNull();
  });

  it("no refusal at all: no", () => {
    expect(admissionRefused(membersReply({ rows: [selfRow(), off({ last_refusal: null })] }))).toBeNull();
  });

  it.each(["me_excluded", "me_removed", "me_pending"])("%s: no", (flag) => {
    expect(admissionRefused(membersReply({ rows: [selfRow(), off()], [flag]: true }))).toBeNull();
  });

  it("empty and missing lists: no", () => {
    expect(admissionRefused(membersReply({ rows: [] }))).toBeNull();
    expect(admissionRefused(null)).toBeNull();
    expect(admissionRefused(undefined)).toBeNull();
  });
});
