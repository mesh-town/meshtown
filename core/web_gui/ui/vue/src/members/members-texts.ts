/* The words of the "members" part of the page, verbatim from the UI spec. A name passed to a builder is already resolved. */

import { NET_NAME, quotedNet } from "../common/words.ts";
import { forDevice } from "../common/device.ts";

export const BADGE_BLOCKED = "Blocked on this computer";
export const BADGE_PENDING = "Waiting to join";
export const BADGE_UNLISTED = "Not a member";
export const LINE_CANT_CONNECT = "Can't connect to you or open sites through you";

export const ACTION_ADMIT = "Confirm…";
export const ACTION_UNBLOCK = "Unblock";

export const MENU_BLOCK = "Block on this computer…";
export const MENU_UNBLOCK = "Unblock";
export const MENU_RENAME = "Rename";
export const MENU_LEAVE = "Leave network…";
export const REASON_LABEL = "Reason (optional, members will see it)";
export const TITLE_BLOCKED = "Already blocked";
export const AFFECTS_EVERYONE = "Affects everyone in the network";

export const DIALOG_CANCEL = "Cancel";
export const CHIP_TITLE = "Members need your confirmation";
export const PENDING_TITLE = "Waiting to join the network";
export const EXCLUDED_BANNED_TEXT =
  "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, ask an admin of the network.";
export const EXCLUDED_REMOVED_TEXT =
  "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, ask an admin of the network for a new invite.";
export const EXCLUDED_LEFT_TEXT = "To use " + NET_NAME + " again, join a network with an invite from one of its members.";
export const EXCLUDED_JOIN = "Join with another invite";
// In the Android app: the way back is the app's own "Leave network and reset…" menu item, reached through this address.
export const EXCLUDED_APP_TEXT = "This phone was removed from the network. Leave the network and reset the app to join with a new invite.";
export const EXCLUDED_APP_BUTTON = "Leave network and reset…";
export const START_OVER_URL = "meshtown-app://start-over";

// The hint above the member list while the creator is the only admin.
export const SECOND_ADMIN_HINT =
  "You are the only admin of this network. Make another member an admin (⋯ → Make admin) so the network can still be managed if this computer is lost.";

export const unblockAria = (name: string): string => "Unblock " + name;
export const moreActionsLabel = (name: string): string => "More actions for " + name;
export const isYourExitText = (name: string): string =>
  name + " is your exit: after this, sites won't open until you choose another exit.";

export const excludedSummary = (n: number): string => "No longer in the network (" + n + ")";
export const unlistedSummary = (n: number): string => "Not in the member list (" + n + ")";

export const bannedStatus = (name: string): string => name + " was removed from the network.";
export const leftStatus = (name: string): string => name + " left the network.";
export const blockedStatus = (name: string): string => name + forDevice(" is blocked on this computer.");

export const stripVoteShortText = (n: number): string => n + " to confirm";
export const stripVoteText = (n: number): string => (n === 1 ? "1 request to confirm" : n + " requests to confirm");
export const chipLabel = (n: number): string => (n === 1 ? "1 request needs your confirmation" : n + " requests need your confirmation");

export const meWaitingText = (): string => forDevice("This computer is waiting for an admin to let it in.");
export const admittedStatus = (name: string): string => name + " joined the network.";
export const labelLine = (label: string): string => "Invited as \"" + label + "\"";
export const admitAria = (name: string): string => "Confirm joining: " + name;

// `net` without quotes; absent or blank — no quoted part.
const EXCLUDED_TITLES = {
  left: "You left the network",
  banned: "This computer was removed from the network",
};
export function excludedTitle(variant: "banned" | "left", net?: string | null): string {
  return forDevice(EXCLUDED_TITLES[variant]) + quotedNet(net);
}

// The header line of a computer that is no longer in the network; `net` without quotes.
export const bannedStatusText = (net?: string | null): string =>
  "Removed from the network" + quotedNet(net) + " by an admin — sites won't open through " + NET_NAME;

// Under the list after leaving: 'You left the network "x".'
const NOT_TOLD_LEFT = "Mesh Town couldn't let the other members know, so for a while they may still see this computer in their list as offline. You don't need to do anything.";
export const leftNetworkStatus = (net?: string | null, announced = true): string =>
  excludedTitle("left", net) + "." + (announced ? "" : " " + forDevice(NOT_TOLD_LEFT));
export const leaveMultiTitle = (net: string): string => `Leave "${net}"?`;
export const leaveMultiBody = (net: string): string =>
  forDevice(`Disconnect this computer from "${net}" for good. Your other networks stay. You'll need a new invite to come back.`);
export const leaveExitLine = (exit: string): string =>
  `Your exit ${exit} is in this network. After you leave, sites will open directly until you choose another exit.`;

// The line under the list after "Make admin".
export const madeAdminStatus = (name: string, net: string): string => name + " is now an admin of " + quotedNet(net).trim();
