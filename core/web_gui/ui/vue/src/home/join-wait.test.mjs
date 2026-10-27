/* The join-by-invite waiting line (`join-wait.ts`): `node.invite_join` → the `#join-wait` text. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { inviteWaitText, refusedText } from "./join-wait.ts";

const info = (over) => ({ state: "waiting", message: "waiting", label: null, expires_unix: 1, ...over });

describe("inviteWaitText", () => {
  it("no join (null, undefined) — empty string", () => {
    assert.equal(inviteWaitText(null), "");
    assert.equal(inviteWaitText(undefined), "");
  });

  it("waiting without a label — a hint without the network name", () => {
    assert.equal(inviteWaitText(info()),
      "Waiting for members of the network to come online. As soon as one of the members from the invite is online, Mesh Town will join the network.");
  });

  it("waiting with a label — the network name in quotes", () => {
    assert.equal(inviteWaitText(info({ label: "family" })),
      "Waiting for members of the network \"family\" to come online. As soon as one of the members from the invite is online, Mesh Town will join the network.");
  });

  it("refused because the invite was used — the core text (RedeemError::Used) verbatim", () => {
    const message = "Another computer has already joined with this invite — it is single-use. Ask whoever sent it for a new one.";
    assert.equal(inviteWaitText(info({ state: "refused", message })), message);
  });

  it("refused because the invite was not made by an admin — the core text (RedeemError::NotFromAdmin) verbatim", () => {
    const message = "This invite can't be used: whoever made it is not a network admin now. Ask a network admin for a new one.";
    assert.equal(inviteWaitText(info({ state: "refused", message })), message);
  });

  it("unknown state — empty string", () => {
    assert.equal(inviteWaitText(info({ state: "other" })), "");
  });
});

describe("refusedText", () => {
  it("is the node message for a refusal and null otherwise", () => {
    assert.equal(refusedText(info({ state: "refused", message: "No." })), "No.");
    assert.equal(refusedText(info()), null);
    assert.equal(refusedText(null), null);
  });
});
