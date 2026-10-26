/* Texts of the "Join or create another network" form and of the banners shown after a network is added. */

import { HOME_CREATE_BTN, HOME_CREATE_LABEL } from "./home-card-texts.ts";

/** The most networks the node holds at once. */
export const MAX_NETS = 8;

export const ADD_NET_TITLE = "Join or create another network";
export const ADD_NET_LINK = "Join or create another network";
export const ADD_NET_TAB_JOIN = "Join";
export const ADD_NET_TAB_CREATE = "Create";
export const ADD_NET_TABS_LABEL = "Join or create";
export const ADD_NET_CREATE_LABEL = HOME_CREATE_LABEL;
export const ADD_NET_CREATE_PLACEHOLDER = "for example, family";
export const ADD_NET_BTN = "Add network";
export const ADD_NET_CREATE_BTN = HOME_CREATE_BTN;
export const ADD_NET_CANCEL = "Cancel";
export const ADD_NET_WAITING = "Joining the network…";
export const ADD_NET_WAITING_SUB = "This takes up to a minute. Your other networks keep working.";
export const JOIN_NO_ANSWER = "No answer yet — the network's host may be offline.";
export const JOIN_CANCEL_BTN = "Cancel";
/** The button of a refused join: it gives the slot back. */
export const JOIN_BACK_BTN = "Back";
export const JOIN_CANCEL_FAILED = "Couldn't cancel — try again.";
/** How long a join waits before the page says nobody has answered. */
export const JOIN_NO_ANSWER_MS = 60_000;
export const ADD_NET_LIMIT = `This invite is for a new network, and Mesh Town is already in ${MAX_NETS}. Leave a network, then paste the invite again.`;
export const ADD_NET_BUSY = "Adding…";
export const ADD_NET_ACTION = "add the network";
export const BANNER_OK = "OK";

export const duplicateText = (net: string): string => `You're already in "${net}" — this invite isn't needed. Keep it for someone else.`;
export const openNetText = (net: string): string => `Open "${net}"`;

// 'You stay in "a".' · '"a" and "b"' · '"a", "b" and "c"'.
export function stayText(names: string[]): string {
  const quoted = names.map((n) => `"${n}"`);
  const last = quoted.pop();
  const list = last === undefined ? "" : quoted.length === 0 ? last : `${quoted.join(", ")} and ${last}`;
  return `You stay in ${list}. The new network gets its own tab in Networks.`;
}

/** `via` — the exit the computer keeps using, when it is not direct: the member and the network it is in.
 * `admins` — the display names of the admins of the network (absent while they are not known). */
export function joinedBanner(net: string, via: { peer: string; net: string } | null, admins?: string[]): string {
  const managed = admins && admins.length > 0 ? ` Only admins invite and remove members here. Admins: ${admins.join(", ")}.` : "";
  const base = `You joined "${net}".${managed}`;
  return via ? `${base} Your exit hasn't changed: sites still open via ${via.peer} (${via.net}).` : base;
}

const NOBODY_YET = (net: string): string => `Nobody else is here yet: use Invite to "${net}" to add a member.`;

/** The banner after creating a network: the creator is its admin and should name a second one. */
export function createdBanner(net: string): string {
  return `You created "${net}" and you're its admin. ${NOBODY_YET(net)} Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.`;
}
