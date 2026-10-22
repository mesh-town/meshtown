/* The words of the "members" part of the page, verbatim from the UI spec. A name passed to a builder is already resolved. */

import { NET_NAME, quotedNet } from "../common/words.ts";

export const BADGE_PROPOSED = "Removal proposed";
export const BADGE_BLOCKED = "Blocked on this computer";
export const BADGE_PENDING = "Waiting to join";
export const BADGE_UNLISTED = "Not a member";
export const LINE_CANT_CONNECT = "Can't connect to you or open sites through you";

export const ACTION_CONFIRM = "Confirm removal…";
export const ACTION_ADMIT = "Confirm…";
export const ACTION_UNBLOCK = "Unblock";

export const MENU_BLOCK = "Block on this computer…";
export const MENU_UNBLOCK = "Unblock";
export const MENU_REMOVE = "Remove from network…";
export const MENU_RENAME = "Rename";
export const MENU_LEAVE = "Leave network…";
export const MENU_WITHDRAW = "Withdraw proposal";
export const REASON_LABEL = "Reason (optional, members will see it)";
export const MENU_UNADMIT = "Withdraw confirmation";
export const MENU_WAITING = "Waiting for another member to confirm";
export const TITLE_PROPOSED = "Removal already proposed";
export const TITLE_BLOCKED = "Already blocked";
export const AFFECTS_EVERYONE = "Affects everyone in the network";

export const DIALOG_CANCEL = "Cancel";
export const VOTE_CONFIRM = "Confirm removal…";
export const VOTE_LATER = "Not now";
export const CHIP_TITLE = "Members need your confirmation";
export const PENDING_TITLE = "Waiting to join the network";
export const EXCLUDED_REMOVED_TEXT =
  "Members of the network removed it. Sites no longer open through Mesh Town, and members can't reach this computer. If you think this is a mistake, contact a member of the network.";
export const EXCLUDED_LEFT_TEXT = "To use " + NET_NAME + " again, join a network with an invite from one of its members.";
export const EXCLUDED_JOIN = "Join with another invite";
// In the Android app: the way back is the app's own "Start over" dialog, reached through this address.
export const EXCLUDED_APP_TEXT = "This phone was removed from the network. Start over to join with a new invite.";
export const EXCLUDED_APP_BUTTON = "Start over";
export const START_OVER_URL = "meshtown-app://start-over";

const plural = (n: number, one: string, many: string): string => n + " " + (n === 1 ? one : many);

export const waitingMoreText = (n: number): string =>
  "You proposed removing it · waiting for " + plural(n, "more member", "more members") + " to confirm";
export const proposedByText = (by: string, n: number): string =>
  "Proposed by " + by + " · needs " + plural(n, "more confirmation", "more confirmations");

export const confirmAria = (name: string): string => "Confirm removal: " + name;
export const unblockAria = (name: string): string => "Unblock " + name;
export const moreActionsLabel = (name: string): string => "More actions for " + name;
export const isYourExitText = (name: string): string =>
  name + " is your exit: after this, sites won't open until you choose another exit.";

export const removedSummary = (n: number): string => "No longer in the network (" + n + ")";
export const unlistedSummary = (n: number): string => "Not in the member list (" + n + ")";

export const removalProposedStatus = (name: string): string =>
  "Removal proposed. " + name + " is already blocked on this computer. Ask another member to open Mesh Town and confirm on the Members tab.";
export const removedStatus = (name: string): string => name + " was removed from the network.";
export const leftStatus = (name: string): string => name + " left the network.";
export const withdrawnStatus = (name: string, stillBlocked: boolean): string =>
  "Removal proposal withdrawn." + (stillBlocked ? "" : " " + name + " is no longer blocked on this computer.");
export const reasonLine = (by: string, reason: string): string => "Reason from " + by + ": \"" + reason + "\"";
export const blockedStatus = (name: string): string => name + " is blocked on this computer.";

export const voteTitle = (by: string, target: string): string => by + " proposes removing " + target + " from the network";
export const voteBody = (target: string): string =>
  "If you confirm, " + target + " is removed for everyone and its open connections are cut. If you don't answer, nothing changes for you: " +
  target + " stays in the network until another member confirms.";

export const stripVoteShortText = (n: number): string => n + " to confirm";
export const stripVoteText = (n: number): string => (n === 1 ? "1 request to confirm" : n + " requests to confirm");
export const chipLabel = (n: number): string => (n === 1 ? "1 request needs your confirmation" : n + " requests need your confirmation");

export const admissionLine = (votes: number, need: number): string => "Confirmed by " + votes + " of " + need;
export const meWaitingText = (votes: number, need: number): string =>
  "This computer is waiting for network members to confirm it (" + votes + " of " + need + ").";
export const admittedStatus = (name: string): string => name + " joined the network.";
export const confirmedStatus = (name: string): string => "You confirmed " + name + ". It joins once another member confirms.";
export const unadmittedStatus = (name: string): string => "Your confirmation for " + name + " was withdrawn.";
export const labelLine = (label: string): string => "Invited as \"" + label + "\"";
export const admitAria = (name: string): string => "Confirm joining: " + name;

// `net` without quotes; absent or blank — no quoted part.
export function excludedTitle(variant: "removed" | "left", net?: string | null): string {
  const tail = quotedNet(net);
  return variant === "left" ? "You left the network" + tail : "This computer was removed from the network" + tail;
}

// The header line of a computer that is no longer in the network; `net` without quotes.
export const excludedStatusText = (net?: string | null): string =>
  "Removed from the network" + quotedNet(net) + " \u2014 sites won't open through " + NET_NAME;

// Under the list after leaving: 'You left the network "x".'
export const leftNetworkStatus = (net?: string | null): string => excludedTitle("left", net) + ".";
