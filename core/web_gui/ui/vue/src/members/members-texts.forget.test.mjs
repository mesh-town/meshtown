/* `members-texts.ts`: the removal texts. The contract is in `members-texts.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as t from "./members-texts.ts";

describe("removal texts", () => {
  it("menu items and marks", () => {
    assert.equal(t.MENU_FORGET, "Remove from network\u2026");
    assert.equal(t.MENU_CONFIRM_FORGET, "Confirm removal\u2026");
    assert.equal(t.MENU_UNDO_FORGET, "Undo removal");
    assert.equal(t.BADGE_FORGET_PROPOSED, "Removal proposed");
    assert.equal(t.FORGET_WAITING, "Waiting for one more member to confirm");
  });

  it("summaries of the groups count the rows", () => {
    assert.equal(t.staleSummary(2), "Offline for over a week (2)");
    assert.equal(t.forgottenSummary(3), "Removed from the member list (3)");
  });

  it("the line of a removed member", () => {
    assert.equal(t.forgottenLine("peer-one and finch-unwary", "19 Oct"), "Removed by peer-one and finch-unwary on 19 Oct");
  });

  describe("statuses under the list", () => {
    it("a proposal is hidden here and waits for another member", () => {
      assert.equal(
        t.forgetProposedStatus("old-laptop"),
        "Removal proposed. old-laptop is hidden on this computer. Ask another member to open Mesh Town and confirm on the Members tab.",
      );
    });

    it("removed", () => {
      assert.equal(t.forgottenStatus("old-laptop"), "old-laptop was removed from the member list.");
    });

    it("withdrawn", () => {
      assert.equal(t.unforgottenStatus("old-laptop"), "Removal withdrawn. old-laptop is back in the member list.");
    });
  });

  describe("this computer was removed", () => {
    it("the header line names the network when there is one", () => {
      assert.equal(t.forgottenStatusText("family"), 'Removed from the member list "family" \u2014 ask a member for an invite');
    });

    it.each([[undefined], [null], [""], ["  "]])("net %j: no quoted part", (net) => {
      assert.equal(t.forgottenStatusText(net), "Removed from the member list \u2014 ask a member for an invite");
    });

    it("the button", () => {
      assert.equal(t.FORGOTTEN_JOIN, "Join with an invite");
    });

    // The text of the card is pinned to the UI spec.
    it("the text of the card", () => {
      assert.equal(
        t.FORGOTTEN_CARD_TEXT,
        "Members removed it after it had been offline for a while. It can't connect to them until it joins again. Ask a member for a new invite \u2014 this computer keeps its key.",
      );
    });
  });

  it("the warning about a pending ban", () => {
    assert.equal(
      t.banAtRiskText("old-laptop"),
      "A ban is waiting for votes in this network. With old-laptop removed, fewer votes are needed, and it may pass right away.",
    );
  });
});
