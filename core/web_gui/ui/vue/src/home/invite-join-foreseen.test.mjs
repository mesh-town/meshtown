/* `refusalForeseen`: the invite leads back to a Peer network that banned this computer, so the members' refusal is expected. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { refusalForeseen } from "./invite-join-actions.ts";

const nets = [{ id: "a1", me_excluded: true }, { id: "b2", me_excluded: false }];
const reply = (over) => ({ label: "x", kind: "Peer", id_hex: "aa", place: "return", slot: "a1", ...over });

describe("refusalForeseen", () => {
  it("return into a banning Peer slot: foreseen", () => {
    assert.equal(refusalForeseen(reply({}), nets), true);
  });
  it("return into a Managed slot that excluded the computer: not foreseen", () => {
    assert.equal(refusalForeseen(reply({ kind: "Managed" }), nets), false);
  });
  it("return into a slot that did not exclude the computer: not foreseen", () => {
    assert.equal(refusalForeseen(reply({ slot: "b2" }), nets), false);
  });
  it("here and new: not foreseen", () => {
    assert.equal(refusalForeseen(reply({ place: "here" }), nets), false);
    assert.equal(refusalForeseen(reply({ place: "new", slot: undefined }), nets), false);
  });
  it("no reply: not foreseen", () => {
    assert.equal(refusalForeseen(null, nets), false);
  });
});
