/* Members texts inside the Android app (the user agent carries `MeshTownApp/`): about this device they say "phone" instead of
 * "computer" — the Leave dialog, the blocked exit, the reasons of a failed action. About another node they keep "computer"
 * (`not_member`). Each text is paired with the desktop wording, which does not change. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { PEER_FULL } from "../testing/section.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { dialogSpec } from "./dialog-specs.ts";
import { exitWarning } from "./exit-members.ts";
import { memberErrorText } from "./members-api.ts";
import { leaveMultiBody } from "./members-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const TAIL = " — sites won't open until you choose another exit";
const REASON = "Couldn't update the member list: ";

describe("leaveMultiBody", () => {
  it("in the app: this phone is disconnected from the network", () => {
    given(APP_UA);
    assert.equal(leaveMultiBody("my devices"), "Disconnect this phone from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.");
  });

  it("on a desktop: this computer, as before", () => {
    given(DESKTOP_UA);
    assert.equal(leaveMultiBody("my devices"), "Disconnect this computer from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.");
  });
});

describe("the Leave dialog", () => {
  describe("one network", () => {
    it("in the app: this phone disconnects from all members", () => {
      given(APP_UA);
      assert.deepEqual(leave().body, [
        "This phone disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
      ]);
    });

    it("in the app: the title and the button are the same", () => {
      given(APP_UA);
      assert.equal(leave().title, "Leave the network \"family\"?");
      assert.equal(leave().action, "Leave network");
      assert.equal(leave().danger, true);
    });

    it("on a desktop: this computer disconnects from all members, as before", () => {
      given(DESKTOP_UA);
      assert.deepEqual(leave().body, [
        "This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
      ]);
    });
  });

  describe("several networks", () => {
    it("in the app: the body says this phone", () => {
      given(APP_UA);
      assert.deepEqual(leave({ multi: true, net: "my devices" }).body, [
        "Disconnect this phone from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.",
      ]);
    });

    it("on a desktop: the body says this computer", () => {
      given(DESKTOP_UA);
      assert.deepEqual(leave({ multi: true, net: "my devices" }).body, [
        "Disconnect this computer from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.",
      ]);
    });
  });
});

describe("the chosen exit is blocked", () => {
  it("in the app: the warning and the hint say \"on this phone\"", () => {
    given(APP_UA);
    const warning = exitWarning([PEER_FULL], blockedExit(), false);
    assert.equal(warning.kind, "blocked");
    assert.equal(warning.text, "Your exit peer-one is blocked on this phone" + TAIL);
    assert.equal(warning.hint, "peer-one is blocked on this phone. Choose another exit below.");
  });

  it("on a desktop: \"on this computer\", as before", () => {
    given(DESKTOP_UA);
    const warning = exitWarning([PEER_FULL], blockedExit(), false);
    assert.equal(warning.text, "Your exit peer-one is blocked on this computer" + TAIL);
    assert.equal(warning.hint, "peer-one is blocked on this computer. Choose another exit below.");
  });
});

describe("the reason of a failed member action (a code without a message)", () => {
  describe("in the app", () => {
    it("self_target: you can't remove your own phone", () => {
      given(APP_UA);
      assert.equal(memberErrorText(new ApiError(409, "self_target")), REASON + "you can't remove your own phone");
    });

    it("left: this phone has left the network", () => {
      given(APP_UA);
      assert.equal(memberErrorText(new ApiError(409, "left")), REASON + "this phone has left the network");
    });

    it("not_member: still \"that computer\" — it is about somebody else's node", () => {
      given(APP_UA);
      assert.equal(memberErrorText(new ApiError(409, "not_member")), REASON + "that computer isn't in the member list");
    });
  });

  describe("on a desktop", () => {
    it.each([
      ["self_target", "you can't remove your own computer"],
      ["not_member", "that computer isn't in the member list"],
      ["left", "this computer has left the network"],
    ])("%s: %s", (code, reason) => {
      given(DESKTOP_UA);
      assert.equal(memberErrorText(new ApiError(409, code)), REASON + reason);
    });
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}

function leave(over = {}) {
  return dialogSpec("leave", { name: "old-laptop", net: "family", isMyExit: false, ...over });
}

function blockedExit() {
  return membersReply({ rows: [selfRow(), peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" })] });
}
