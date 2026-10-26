/* `members-texts.ts`: the words of the "members" part of the page (UI spec, sections 1-3), pinned verbatim.
 * Named exports of `./members-texts.ts`; a name passed to a builder is already resolved by the caller. There are no vote words
 * (proposals, confirmations counted by numbers, removal circles): an admin acts alone. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as t from "./members-texts.ts";

describe("marks and row lines", () => {
  it("marks after the name", () => {
    assert.equal(t.BADGE_BLOCKED, "Blocked on this computer");
    assert.equal(t.BADGE_UNLISTED, "Not a member");
    assert.equal(t.BADGE_PENDING, "Waiting to join");
  });

  it("the line of a member blocked by me", () => {
    assert.equal(t.LINE_CANT_CONNECT, "Can't connect to you or open sites through you");
  });
});

describe("row actions and labels", () => {
  it("the main actions", () => {
    assert.equal(t.ACTION_UNBLOCK, "Unblock");
    assert.equal(t.ACTION_ADMIT, "Confirm…");
  });

  it("aria-labels name the member", () => {
    assert.equal(t.unblockAria("old-laptop"), "Unblock old-laptop");
    assert.equal(t.moreActionsLabel("old-laptop"), "More actions for old-laptop");
    assert.equal(t.admitAria("old-laptop"), "Confirm joining: old-laptop");
  });
});

describe("the menu", () => {
  it("items", () => {
    assert.equal(t.MENU_BLOCK, "Block on this computer…");
    assert.equal(t.MENU_UNBLOCK, "Unblock");
    assert.equal(t.MENU_RENAME, "Rename");
    assert.equal(t.MENU_LEAVE, "Leave network…");
  });

  it("tooltips and descriptions", () => {
    assert.equal(t.TITLE_BLOCKED, "Already blocked");
    assert.equal(t.AFFECTS_EVERYONE, "Affects everyone in the network");
  });
});

describe("dialogs", () => {
  it("the cancel button", () => {
    assert.equal(t.DIALOG_CANCEL, "Cancel");
  });

  it("the paragraph for a target that is my exit", () => {
    assert.equal(t.isYourExitText("old-laptop"), "old-laptop is your exit: after this, sites won't open until you choose another exit.");
  });

  it("the label of the reason field", () => {
    assert.equal(t.REASON_LABEL, "Reason (optional, members will see it)");
  });
});

describe("the lists under the members", () => {
  it("excluded members", () => {
    assert.equal(t.excludedSummary(1), "No longer in the network (1)");
    assert.equal(t.excludedSummary(3), "No longer in the network (3)");
  });

  it("nodes that are not in the member list", () => {
    assert.equal(t.unlistedSummary(2), "Not in the member list (2)");
  });
});

describe("the status under the list after an action", () => {
  it("a member was removed", () => {
    assert.equal(t.bannedStatus("old-laptop"), "old-laptop was removed from the network.");
  });

  it("a member left", () => {
    assert.equal(t.leftStatus("old-laptop"), "old-laptop left the network.");
  });

  it("a member was blocked", () => {
    assert.equal(t.blockedStatus("old-laptop"), "old-laptop is blocked on this computer.");
  });

  it("a member joined", () => {
    assert.equal(t.admittedStatus("old-laptop"), "old-laptop joined the network.");
  });
});

describe("the status under the list after leaving", () => {
  it("without the second argument: the sentence it always had", () => {
    assert.equal(t.leftNetworkStatus("family"), 'You left the network "family".');
  });

  it("the others were not told: a second sentence follows", () => {
    const text = t.leftNetworkStatus("family", false);
    assert.ok(text.startsWith('You left the network "family". '), text);
    assert.ok(text.length > t.leftNetworkStatus("family").length);
  });

  it.each([true, false])("announced %j, no network name: no quotes and no \"undefined\"", (announced) => {
    for (const net of [undefined, "", "  "]) {
      const text = t.leftNetworkStatus(net, announced);
      assert.ok(!text.includes('"') && !text.includes("undefined"), text);
    }
  });

  it("the plain status is the title of the left card with a full stop", () => {
    assert.equal(t.leftNetworkStatus("family"), t.excludedTitle("left", "family") + ".");
  });
});

describe("signals that a request needs my confirmation", () => {
  it("the link in the header", () => {
    assert.equal(t.stripVoteText(1), "1 request to confirm");
    assert.equal(t.stripVoteText(5), "5 requests to confirm");
    assert.equal(t.stripVoteShortText(3), "3 to confirm");
  });

  it("the counter on the tab", () => {
    assert.equal(t.chipLabel(1), "1 request needs your confirmation");
    assert.equal(t.chipLabel(7), "7 requests need your confirmation");
    assert.equal(t.CHIP_TITLE, "Members need your confirmation");
  });
});

describe("the screen of an excluded computer", () => {
  it("removed by an admin: the title names the network", () => {
    assert.equal(t.excludedTitle("banned", "family"), 'This computer was removed from the network "family"');
  });

  it("left on its own: the title names the network", () => {
    assert.equal(t.excludedTitle("left", "family"), 'You left the network "family"');
  });

  it.each(["banned", "left"])("%s: without a network name there are no quotes", (variant) => {
    for (const net of [undefined, "", "  "]) {
      const title = t.excludedTitle(variant, net);
      assert.ok(!title.includes('"') && !title.includes("undefined"), variant + " / " + String(net) + ": " + title);
      assert.equal(title, title.trim());
    }
  });

  it("the header line of a removed computer", () => {
    assert.equal(t.bannedStatusText("family"), 'Removed from the network "family" by an admin — sites won\'t open through Mesh Town');
  });

  it("the text and the button", () => {
    assert.equal(
      t.EXCLUDED_REMOVED_TEXT,
      "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, ask an admin of the network for a new invite.",
    );
    assert.equal(t.EXCLUDED_JOIN, "Join with another invite");
  });
});

describe("join request texts", () => {
  it("the waiting computer names an admin, no numbers", () => {
    assert.equal(t.meWaitingText(), "This computer is waiting for an admin to let it in.");
  });

  it("fixed words", () => {
    assert.equal(t.PENDING_TITLE, "Waiting to join the network");
    assert.equal(t.labelLine("Anna"), 'Invited as "Anna"');
  });
});

describe("the hint for the only admin", () => {
  it("says what to do and where", () => {
    assert.equal(
      t.SECOND_ADMIN_HINT,
      "You are the only admin of this network. Make another member an admin (⋯ → Make admin) so the network can still be managed if this computer is lost.",
    );
  });
});

describe("no vote or removal-list words anywhere", () => {
  it("no export is about votes, proposals or the member list removal", () => {
    const words = Object.values(t).filter((v) => typeof v === "string");
    for (const text of words) assert.ok(!/propos|\bvote|Peer|forgot/i.test(text), text);
  });
});
