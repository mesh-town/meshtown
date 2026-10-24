/* `members-texts.ts`: the words of the "members" part of the page (UI spec, sections 1-3), pinned verbatim.
 *
 * Contract fixed by these tests — named exports of `./members-texts.ts` (the names are the test author's choice):
 *  constants (strings):
 *    BADGE_BAN_PROPOSED, BADGE_BLOCKED, BADGE_UNLISTED      marks after a name
 *    LINE_CANT_CONNECT                                  row line of a member blocked by me
 *    ACTION_CONFIRM_BAN, ACTION_UNBLOCK                     main actions of a row
 *    MENU_BLOCK, MENU_BAN, MENU_RENAME, MENU_LEAVE, MENU_WAITING   items of the `⋯` menu
 *    TITLE_BAN_PROPOSED, TITLE_BLOCKED                      tooltips of disabled menu items
 *    AFFECTS_EVERYONE                                   hidden description of "Ban from network…"
 *    DIALOG_CANCEL                                      the cancel button of every dialog
 *    VOTE_CONFIRM_BAN, VOTE_LATER                           buttons of the banner
 *    CHIP_TITLE                                         tooltip of the counter on the tab
 *    EXCLUDED_BANNED_TEXT, EXCLUDED_JOIN               the "banned" card: body and button
 *  builders (functions returning strings; a name is passed already resolved by the caller):
 *    waitingMoreText(n), proposedByText(by, n)          row lines of a proposal (n = missing votes, plural by n)
 *    confirmAria(name), unblockAria(name), moreActionsLabel(name)    aria-labels
 *    isYourExitText(name)                               the extra paragraph of the dialogs
 *    excludedSummary(n), unlistedSummary(n)              summaries of the two `<details>`
 *    banProposedStatus(name), bannedStatus(name), leftStatus(name), blockedStatus(name)   `#peers-exit-status` after an action
 *    MENU_WITHDRAW                                      the item that takes my own proposal back
 *    withdrawnStatus(name, stillBlocked)                `#peers-exit-status` after taking a proposal back
 *    REASON_LABEL, reasonLine(by, reason)               the optional reason of a ban: the field label and the line shown to members
 *    voteTitle(by, target), voteBody(target)            the banner
 *    stripVoteText(n)                                   the link in the header: 1 -> singular, n >= 2 -> "N requests need ..."
 *    chipLabel(n)                                       aria-label of the counter on the tab (same wording as the header)
 *    excludedTitle(variant, net)                        variant "banned" | "left"; `net` without quotes, absent/blank -> no quotes part
 *  removal: MENU_FORGET, MENU_CONFIRM_FORGET, MENU_UNDO_FORGET, BADGE_FORGET_PROPOSED, FORGET_WAITING, FORGOTTEN_CARD_TEXT, FORGOTTEN_JOIN,
 *    staleSummary(n), forgottenSummary(n), forgottenLine(by, date), lastOnlineText(sec | null), forgetProposedStatus(name),
 *    forgottenStatus(name), unforgottenStatus(name), forgottenStatusText(net), banAtRiskText(name)
 * Not covered (not defined by the spec): the body text of the "left" card, the warnings about a chosen exit. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as t from "./members-texts.ts";

describe("marks and row lines", () => {
  it("marks after the name", () => {
    assert.equal(t.BADGE_BAN_PROPOSED, "Ban proposed");
    assert.equal(t.BADGE_BLOCKED, "Blocked on this computer");
    assert.equal(t.BADGE_UNLISTED, "Not a member");
  });

  it("the line of a member blocked by me", () => {
    assert.equal(t.LINE_CANT_CONNECT, "Can't connect to you or open sites through you");
  });

  describe("my own proposal is waiting", () => {
    it("one more member", () => {
      assert.equal(t.waitingMoreText(1), "You proposed banning it · waiting for 1 more member to confirm");
    });

    it("several more members", () => {
      assert.equal(t.waitingMoreText(2), "You proposed banning it · waiting for 2 more members to confirm");
    });
  });

  describe("a proposal of another member", () => {
    it("one more confirmation", () => {
      assert.equal(t.proposedByText("peer-one", 1), "Proposed by peer-one · needs 1 more confirmation");
    });

    it("several more confirmations", () => {
      assert.equal(t.proposedByText("peer-one", 2), "Proposed by peer-one · needs 2 more confirmations");
    });
  });
});

describe("row actions and labels", () => {
  it("the main actions", () => {
    assert.equal(t.ACTION_CONFIRM_BAN, "Confirm ban…");
    assert.equal(t.ACTION_UNBLOCK, "Unblock");
  });

  it("aria-labels name the member", () => {
    assert.equal(t.confirmAria("old-laptop"), "Confirm ban: old-laptop");
    assert.equal(t.unblockAria("old-laptop"), "Unblock old-laptop");
    assert.equal(t.moreActionsLabel("old-laptop"), "More actions for old-laptop");
  });
});

describe("the menu", () => {
  it("items", () => {
    assert.equal(t.MENU_BLOCK, "Block on this computer…");
    assert.equal(t.MENU_BAN, "Ban from network…");
    assert.equal(t.MENU_RENAME, "Rename");
    assert.equal(t.MENU_LEAVE, "Leave network…");
    assert.equal(t.MENU_WAITING, "Waiting for another member to confirm");
  });

  it("the item that takes my own proposal back", () => {
    assert.equal(t.MENU_WITHDRAW, "Withdraw proposal");
  });

  it("tooltips of the disabled items", () => {
    assert.equal(t.TITLE_BAN_PROPOSED, "Ban already proposed");
    assert.equal(t.TITLE_BLOCKED, "Already blocked");
  });

  it("the hidden description of the dangerous item", () => {
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
  it("a ban was proposed", () => {
    assert.equal(
      t.banProposedStatus("old-laptop"),
      "Ban proposed. old-laptop is already blocked on this computer. Ask another member to open Mesh Town and confirm on the Members tab.",
    );
  });

  it("a member was banned", () => {
    assert.equal(t.bannedStatus("old-laptop"), "old-laptop was banned from the network.");
  });

  it("a member left", () => {
    assert.equal(t.leftStatus("old-laptop"), "old-laptop left the network.");
  });

  it("a member was blocked", () => {
    assert.equal(t.blockedStatus("old-laptop"), "old-laptop is blocked on this computer.");
  });
});

describe("the status under the list after leaving", () => {
  const LEFT = 'You left the network "my devices".';
  const NOT_TOLD = "Mesh Town couldn't let the other members know, so for a while they may still see this computer in their list as offline. You don't need to do anything.";

  it("without the second argument: the sentence it always had", () => {
    assert.equal(t.leftNetworkStatus("my devices"), LEFT);
  });

  it("the others were told: the same single sentence", () => {
    assert.equal(t.leftNetworkStatus("my devices", true), LEFT);
  });

  it("the others were not told: a second sentence follows", () => {
    assert.equal(t.leftNetworkStatus("my devices", false), LEFT + " " + NOT_TOLD);
  });

  it.each([true, false])("announced %j, no network name: no quotes and no \"undefined\"", (announced) => {
    for (const net of [undefined, null, "", "  "]) {
      const text = t.leftNetworkStatus(net, announced);
      assert.ok(text.startsWith("You left the network."), String(net) + ": " + text);
      assert.ok(!/undefined|null|"/.test(text), String(net) + ": " + text);
    }
  });

  it("the plain status is the title of the left card with a full stop", () => {
    assert.equal(t.leftNetworkStatus("family"), t.excludedTitle("left", "family") + ".");
  });
});

describe("taking a proposal back", () => {
  it("the member is no longer blocked here: the status says so", () => {
    assert.equal(
      t.withdrawnStatus("old-laptop", false),
      "Ban proposal withdrawn. old-laptop is no longer blocked on this computer.",
    );
  });

  it("the member is still blocked here (blocked separately): only the withdrawal is reported", () => {
    assert.equal(t.withdrawnStatus("old-laptop", true), "Ban proposal withdrawn.");
  });
});

describe("the reason of a ban", () => {
  it("the label of the field in the dialog", () => {
    assert.equal(t.REASON_LABEL, "Reason (optional, members will see it)");
  });

  it("the line shown to the other members names the proposer and quotes the reason", () => {
    assert.equal(t.reasonLine("peer-one", "lost laptop"), 'Reason from peer-one: "lost laptop"');
  });

  it("the reason is put in as it is", () => {
    assert.equal(t.reasonLine("peer-one", "<b>x</b> & \"y\""), 'Reason from peer-one: "<b>x</b> & "y""');
  });
});

describe("the banner on the Members tab", () => {
  it("the title names the proposer and the target", () => {
    assert.equal(t.voteTitle("peer-one", "old-laptop"), "peer-one proposes banning old-laptop from the network");
  });

  it("the text says what a confirmation and a silence do", () => {
    assert.equal(
      t.voteBody("old-laptop"),
      "If you confirm, old-laptop is banned for everyone and its open connections are cut. If you don't answer, nothing changes for you: old-laptop stays in the network until another member confirms.",
    );
  });

  it("the buttons", () => {
    assert.equal(t.VOTE_CONFIRM_BAN, "Confirm ban…");
    assert.equal(t.VOTE_LATER, "Not now");
  });
});

describe("signals that a request needs my confirmation", () => {
  describe("the link in the header", () => {
    it("one ban", () => {
      assert.equal(t.stripVoteText(1), "1 request to confirm");
    });

    it("several bans", () => {
      assert.equal(t.stripVoteText(2), "2 requests to confirm");
      assert.equal(t.stripVoteText(5), "5 requests to confirm");
    });
  });

  describe("the counter on the tab", () => {
    it("one ban", () => {
      assert.equal(t.chipLabel(1), "1 request needs your confirmation");
    });

    it("several bans", () => {
      assert.equal(t.chipLabel(2), "2 requests need your confirmation");
      assert.equal(t.chipLabel(7), "7 requests need your confirmation");
    });

    it("the tooltip", () => {
      assert.equal(t.CHIP_TITLE, "Members need your confirmation");
    });
  });
});

describe("the screen of an excluded computer", () => {
  it("banned by the members: the title names the network", () => {
    assert.equal(t.excludedTitle("banned", "family"), 'This computer was banned from the network "family"');
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

  it("the text for \"banned\"", () => {
    assert.equal(
      t.EXCLUDED_BANNED_TEXT,
      "Members of the network banned it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, contact a member of the network.",
    );
  });

  it("the text for a removal by an admin in a Managed network", () => {
    assert.equal(
      t.EXCLUDED_REMOVED_TEXT,
      "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, contact a member of the network.",
    );
  });

  it("the button", () => {
    assert.equal(t.EXCLUDED_JOIN, "Join with another invite");
  });
});

describe("join request texts", () => {
  it("admissionLine counts confirmations", () => {
    assert.equal(t.admissionLine(1, 2), "Confirmed by 1 of 2");
    assert.equal(t.admissionLine(0, 1), "Confirmed by 0 of 1");
  });

  it("meWaitingText names both numbers", () => {
    assert.equal(t.meWaitingText(0, 2), "This computer is waiting for network members to confirm it (0 of 2).");
  });

  it("fixed words", () => {
    assert.equal(t.BADGE_PENDING, "Waiting to join");
    assert.equal(t.ACTION_ADMIT, "Confirm…");
    assert.equal(t.MENU_UNADMIT, "Withdraw confirmation");
    assert.equal(t.PENDING_TITLE, "Waiting to join the network");
  });
});

describe("ban texts do not use the removal wording", () => {
  const BAN_STRINGS = [
    t.BADGE_BAN_PROPOSED, t.ACTION_CONFIRM_BAN, t.VOTE_CONFIRM_BAN, t.MENU_BAN, t.TITLE_BAN_PROPOSED, t.EXCLUDED_BANNED_TEXT,
    t.EXCLUDED_APP_TEXT, t.AFFECTS_EVERYONE,
  ];
  const BAN_BUILT = [
    t.waitingMoreText(1), t.confirmAria("old-laptop"), t.banProposedStatus("old-laptop"), t.bannedStatus("old-laptop"),
    t.withdrawnStatus("old-laptop", false), t.withdrawnStatus("old-laptop", true), t.voteTitle("peer-one", "old-laptop"),
    t.voteBody("old-laptop"), t.excludedTitle("banned", "family"), t.bannedStatusText("family"),
  ];

  it("no constant or builder of the ban says remove, removal or removed", () => {
    for (const text of [...BAN_STRINGS, ...BAN_BUILT]) assert.ok(!/remov/i.test(text), text);
  });

  it("the ban words themselves", () => {
    assert.equal(t.MENU_BAN, "Ban from network\u2026");
    assert.equal(t.ACTION_CONFIRM_BAN, "Confirm ban\u2026");
    assert.equal(t.VOTE_CONFIRM_BAN, "Confirm ban\u2026");
    assert.equal(t.TITLE_BAN_PROPOSED, "Ban already proposed");
    assert.equal(t.BADGE_BAN_PROPOSED, "Ban proposed");
    assert.equal(t.EXCLUDED_APP_TEXT, "This phone was banned from the network. Start over to join with a new invite.");
    assert.equal(t.confirmAria("old-laptop"), "Confirm ban: old-laptop");
    assert.equal(t.waitingMoreText(1), "You proposed banning it \u00b7 waiting for 1 more member to confirm");
  });

  it("statuses after a ban", () => {
    assert.equal(t.bannedStatus("old-laptop"), "old-laptop was banned from the network.");
    assert.ok(t.banProposedStatus("old-laptop").startsWith("Ban proposed. old-laptop is already blocked on this computer."), t.banProposedStatus("old-laptop"));
    assert.ok(t.withdrawnStatus("old-laptop", false).startsWith("Ban proposal withdrawn."), t.withdrawnStatus("old-laptop", false));
  });

  it("the banner", () => {
    assert.equal(t.voteTitle("peer-one", "old-laptop"), "peer-one proposes banning old-laptop from the network");
    assert.ok(t.voteBody("old-laptop").startsWith("If you confirm, old-laptop is banned for everyone"), t.voteBody("old-laptop"));
  });

  it("the titles and the header line of a computer that was banned", () => {
    assert.equal(t.excludedTitle("banned", "family"), 'This computer was banned from the network "family"');
    assert.equal(t.excludedTitle("banned"), "This computer was banned from the network");
    assert.ok(t.bannedStatusText("family").startsWith('Banned from the network "family"'), t.bannedStatusText("family"));
  });
});

describe("lastOnlineText", () => {
  it.each([
    [777600, "Last online 9 d ago"],
    [86400, "Last online 1 d ago"],
    [7200, "Last online 2 h ago"],
    [300, "Last online 5 min ago"],
    [10, "Last online just now"],
    [0, "Last online just now"],
  ])("%s s: %s", (sec, expected) => {
    assert.equal(t.lastOnlineText(sec), expected);
  });

  it("never seen (null)", () => {
    assert.equal(t.lastOnlineText(null), "Never seen online");
  });
});
