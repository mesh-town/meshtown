/* The words of the "members" part of the page, verbatim from the UI spec. A name passed to a builder is already resolved. */

import { NET_NAME, agoText, quotedNet } from "../common/words.ts";
import { forDevice } from "../common/device.ts";

export const BADGE_BAN_PROPOSED = "Ban proposed";
export const BADGE_BLOCKED = "Blocked on this computer";
export const BADGE_PENDING = "Waiting to join";
export const BADGE_UNLISTED = "Not a member";
export const LINE_CANT_CONNECT = "Can't connect to you or open sites through you";

export const ACTION_CONFIRM_BAN = "Confirm ban…";
export const ACTION_ADMIT = "Confirm…";
export const ACTION_UNBLOCK = "Unblock";

export const MENU_BLOCK = "Block on this computer…";
export const MENU_UNBLOCK = "Unblock";
export const MENU_BAN = "Ban from network…";
export const MENU_RENAME = "Rename";
export const MENU_LEAVE = "Leave network…";
export const MENU_WITHDRAW = "Withdraw proposal";
export const REASON_LABEL = "Reason (optional, members will see it)";
export const MENU_UNADMIT = "Withdraw confirmation";
export const MENU_WAITING = "Waiting for another member to confirm";
export const TITLE_BAN_PROPOSED = "Ban already proposed";
export const TITLE_BLOCKED = "Already blocked";
export const AFFECTS_EVERYONE = "Affects everyone in the network";

export const DIALOG_CANCEL = "Cancel";
export const VOTE_CONFIRM_BAN = "Confirm ban…";
export const VOTE_LATER = "Not now";
export const CHIP_TITLE = "Members need your confirmation";
export const PENDING_TITLE = "Waiting to join the network";
export const EXCLUDED_BANNED_TEXT =
  "Members of the network banned it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, contact a member of the network.";
export const EXCLUDED_REMOVED_TEXT =
  "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, ask an admin of the network for a new invite.";
export const EXCLUDED_LEFT_TEXT = "To use " + NET_NAME + " again, join a network with an invite from one of its members.";
export const EXCLUDED_JOIN = "Join with another invite";
// In the Android app: the way back is the app's own "Leave network and reset…" menu item, reached through this address.
export const EXCLUDED_APP_TEXT = "This phone was banned from the network. Leave the network and reset the app to join with a new invite.";
export const EXCLUDED_APP_BUTTON = "Leave network and reset\u2026";
export const START_OVER_URL = "meshtown-app://start-over";

const plural = (n: number, one: string, many: string): string => n + " " + (n === 1 ? one : many);

export const waitingMoreText = (n: number): string =>
  "You proposed banning it · waiting for " + plural(n, "more member", "more members") + " to confirm";
export const proposedByText = (by: string, n: number): string =>
  "Proposed by " + by + " · needs " + plural(n, "more confirmation", "more confirmations");

export const confirmAria = (name: string): string => "Confirm ban: " + name;
export const unblockAria = (name: string): string => "Unblock " + name;
export const moreActionsLabel = (name: string): string => "More actions for " + name;
export const isYourExitText = (name: string): string =>
  name + " is your exit: after this, sites won't open until you choose another exit.";

export const excludedSummary = (n: number): string => "No longer in the network (" + n + ")";
export const unlistedSummary = (n: number): string => "Not in the member list (" + n + ")";

export const banProposedStatus = (name: string): string =>
  "Ban proposed. " + name + forDevice(" is already blocked on this computer. Ask another member to open Mesh Town and confirm on the Members tab.");
export const bannedStatus = (name: string): string => name + " was banned from the network.";
export const leftStatus = (name: string): string => name + " left the network.";
export const withdrawnStatus = (name: string, stillBlocked: boolean): string =>
  "Ban proposal withdrawn." + (stillBlocked ? "" : " " + name + forDevice(" is no longer blocked on this computer."));
export const reasonLine = (by: string, reason: string): string => "Reason from " + by + ": \"" + reason + "\"";
export const blockedStatus = (name: string): string => name + forDevice(" is blocked on this computer.");

export const voteTitle = (by: string, target: string): string => by + " proposes banning " + target + " from the network";
export const voteBody = (target: string): string =>
  "If you confirm, " + target + " is banned for everyone and its open connections are cut. If you don't answer, nothing changes for you: " +
  target + " stays in the network until another member confirms.";

export const stripVoteShortText = (n: number): string => n + " to confirm";
export const stripVoteText = (n: number): string => (n === 1 ? "1 request to confirm" : n + " requests to confirm");
export const chipLabel = (n: number): string => (n === 1 ? "1 request needs your confirmation" : n + " requests need your confirmation");

export const admissionLine = (votes: number, need: number): string => "Confirmed by " + votes + " of " + need;
export const meWaitingText = (votes: number, need: number): string =>
  forDevice("This computer is waiting for network members to confirm it (" + votes + " of " + need + ").");
export const admittedStatus = (name: string): string => name + " joined the network.";
export const confirmedStatus = (name: string): string => "You confirmed " + name + ". It joins once another member confirms.";
export const unadmittedStatus = (name: string): string => "Your confirmation for " + name + " was withdrawn.";
export const labelLine = (label: string): string => "Invited as \"" + label + "\"";
export const admitAria = (name: string): string => "Confirm joining: " + name;

// `net` without quotes; absent or blank — no quoted part.
const EXCLUDED_TITLES = {
  left: "You left the network",
  banned: "This computer was banned from the network",
  forgotten: "This computer was removed from the member list",
};
const REMOVED_TITLE = "This computer was removed from the network";
// `managed` — an admin removed the computer from a Managed network: "removed", not "banned".
export function excludedTitle(variant: "banned" | "left" | "forgotten", net?: string | null, managed = false): string {
  return forDevice(managed && variant === "banned" ? REMOVED_TITLE : EXCLUDED_TITLES[variant]) + quotedNet(net);
}

// The header line of a computer that is no longer in the network; `net` without quotes.
export const bannedStatusText = (net?: string | null, managed = false): string =>
  (managed ? "Removed from the network" : "Banned from the network") + quotedNet(net) + (managed ? " by an admin" : "") +
  " \u2014 sites won't open through " + NET_NAME;

// Under the list after leaving: 'You left the network "x".'
const NOT_TOLD_LEFT = "Mesh Town couldn't let the other members know, so for a while they may still see this computer in their list as offline. You don't need to do anything.";
export const leftNetworkStatus = (net?: string | null, announced = true): string =>
  excludedTitle("left", net) + "." + (announced ? "" : " " + forDevice(NOT_TOLD_LEFT));
export const leaveMultiTitle = (net: string): string => `Leave "${net}"?`;
export const leaveMultiBody = (net: string): string =>
  forDevice(`Disconnect this computer from "${net}" for good. Your other networks stay. You'll need a new invite to come back.`);
export const leaveExitLine = (exit: string): string =>
  `Your exit ${exit} is in this network. After you leave, sites will open directly until you choose another exit.`;

// Removing an offline node from the member list.
export const MENU_FORGET = "Remove from network\u2026";
export const MENU_CONFIRM_FORGET = "Confirm removal\u2026";
export const MENU_UNDO_FORGET = "Undo removal";
export const BADGE_FORGET_PROPOSED = "Removal proposed";
export const FORGET_WAITING = "Waiting for one more member to confirm";
export const FORGOTTEN_CARD_TEXT =
  "Members removed it after it had been offline for a while. It can't connect to them until it joins again. Ask a member for a new invite.";
export const FORGOTTEN_JOIN = "Join with an invite";
export const FORGET_RECENT_HINT = "It was online recently \u2014 remove it only if this computer is gone.";

export const staleSummary = (n: number): string => "Offline for over a week (" + n + ")";
export const forgottenSummary = (n: number): string => "Removed from the member list (" + n + ")";
export const forgottenLine = (by: string, date: string): string => "Removed by " + by + " on " + date;
export const lastOnlineText = (sec: number | null): string => (sec === null ? "Never seen online" : "Last online " + agoText(sec));

export const forgetTitle = (name: string): string => "Remove " + name + " from the network?";
export const forgetLastSeenLine = (name: string, sec: number | null): string =>
  name + (sec === null ? " has never been seen online." : " was last online " + agoText(sec) + ".");
export const forgetEffectLine = (): string =>
  "It disappears from the member list on every computer in the network and can't connect until it joins again. To bring it back, send it a new invite.";
export const forgetNeedTwoLine = (name: string): string =>
  "This takes two members: you propose, and one more member confirms. " + forDevice("On this computer, ") + name + " is hidden right away.";
// With no third member `other` is empty: the two of you are the target and this computer.
export const forgetOnlyTwoLine = (name: string, other: string): string =>
  "You and " + (other || name) + " are the only members, so " + name + " is removed right away.";
export const forgetConfirmTitle = (name: string): string => "Confirm removing " + name + "?";
export const forgetConfirmLine = (name: string): string => "Yours is the second vote: " + name + " is removed for everyone right away.";
export const FORGET_PROPOSE = "Propose removal";
export const forgetAction = (name: string): string => "Remove " + name;
export const banAtRiskText = (name: string): string =>
  "A ban is waiting for votes in this network. With " + name + " removed, fewer votes are needed, and it may pass right away.";

export const forgetProposedStatus = (name: string): string =>
  "Removal proposed. " + name + forDevice(" is hidden on this computer. Ask another member to open Mesh Town and confirm on the Members tab.");
export const forgottenStatus = (name: string): string => name + " was removed from the member list.";
export const unforgottenStatus = (name: string): string => "Removal withdrawn. " + name + " is back in the member list.";
export const forgottenStatusText = (net?: string | null): string =>
  "Removed from the member list" + quotedNet(net) + " \u2014 ask a member for an invite";

// The line under the list after "Make admin".
export const madeAdminStatus = (name: string, net: string): string => name + " is now an admin of " + quotedNet(net).trim();
