/* The invite in the address (`invite-param.ts`). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { inviteFromSearch, withoutInvite } from "./invite-param.ts";

describe("inviteFromSearch", () => {
  it("takes an invite", () => {
    assert.equal(inviteFromSearch("?token=a&invite=mesh-invite2-abc%20def"), "mesh-invite2-abc def");
  });
  it("ignores other values and a missing parameter", () => {
    assert.equal(inviteFromSearch("?invite=hello"), "");
    assert.equal(inviteFromSearch("?token=a"), "");
    assert.equal(inviteFromSearch(""), "");
  });
});

describe("withoutInvite", () => {
  it("keeps the other parameters and the hash", () => {
    assert.equal(withoutInvite("?token=a&invite=mesh-invite2-x", "/", "#home"), "/?token=a#home");
  });
  it("drops the question mark when nothing is left", () => {
    assert.equal(withoutInvite("?invite=mesh-invite2-x", "/", ""), "/");
  });
});
