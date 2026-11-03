/* `parseInvites` and `inviteErrorText`: the reply of `network/invites` and the text of a failed edit. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { inviteErrorText, parseInvites } from "./invites-api.ts";

const row = (over = {}) => ({ hash: "a".repeat(64), for_whom: "Anna", made_unix: 5, expires_unix: 9, issuer: "b".repeat(64), issuer_name: "bob", mine: true, ...over });

describe("parseInvites", () => {
  it("reads a row as sent", () => {
    assert.deepEqual(parseInvites({ invites: [row()] }), [row()]);
  });

  it("an empty or blank for_whom becomes null", () => {
    const got = parseInvites({ invites: [row({ for_whom: "" }), row({ for_whom: "  " }), row({ for_whom: null })] });
    assert.deepEqual(got.map((r) => r.for_whom), [null, null, null]);
  });

  it("drops rows without a hash; not a list is null", () => {
    assert.equal(parseInvites({ invites: [{ for_whom: "x" }] }).length, 0);
    assert.equal(parseInvites({}), null);
    assert.equal(parseInvites(null), null);
  });
});

describe("inviteErrorText", () => {
  it("a known code gets a sentence; another one the node's message", () => {
    assert.equal(inviteErrorText(new ApiError(404, "not_found", "x")), "This invite has been used or has expired.");
    assert.equal(inviteErrorText(new ApiError(500, "internal", "boom")), "Couldn't update the invites: boom");
  });
});
