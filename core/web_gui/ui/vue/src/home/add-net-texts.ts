/* Texts of the "Add another network" form and of the banners shown after a network is added. */

import { HOME_CREATE_BTN, HOME_CREATE_LABEL, HOME_JOIN_FIELD_LABEL, HOME_JOIN_PASTE_HINT, HOME_JOIN_PLACEHOLDER } from "./home-card-texts.ts";

/** The most networks the node holds at once. */
export const MAX_NETS = 8;

export const ADD_NET_TITLE = "Add another network";
export const ADD_NET_LINK = "Add another network";
export const ADD_NET_JOIN_LABEL = HOME_JOIN_FIELD_LABEL;
export const ADD_NET_JOIN_PLACEHOLDER = HOME_JOIN_PLACEHOLDER;
export const ADD_NET_JOIN_HINT = HOME_JOIN_PASTE_HINT;
export const ADD_NET_CREATE_LABEL = HOME_CREATE_LABEL;
export const ADD_NET_CREATE_PLACEHOLDER = "for example, family";
export const ADD_NET_BTN = "Add network";
export const ADD_NET_CREATE_BTN = HOME_CREATE_BTN;
export const ADD_NET_CANCEL = "Cancel";
export const ADD_NET_TO_CREATE = "No invite? Create a new network ▸";
export const ADD_NET_TO_JOIN = "Have an invite? Join a network ▸";
export const ADD_NET_WAITING = "Joining the network…";
export const ADD_NET_WAITING_SUB = "This takes up to a minute. Your other networks keep working.";
export const ADD_NET_LIMIT = `Mesh Town can be in up to ${MAX_NETS} networks at once. To add this one, leave another network first.`;
export const ADD_NET_BUSY = "Adding…";
export const ADD_NET_ACTION = "add the network";
export const ADD_NET_CREATE_ACTION = "create the network";
export const BANNER_OK = "OK";

export const duplicateText = (net: string): string => `You're already in "${net}" — this invite isn't needed. Keep it for someone else.`;
export const openNetText = (net: string): string => `Open "${net}"`;

// 'You stay in "a".' · '"a" and "b"' · '"a", "b" and "c"'.
export function stayText(names: string[]): string {
  const quoted = names.map((n) => `"${n}"`);
  const last = quoted.pop();
  const list = last === undefined ? "" : quoted.length === 0 ? last : `${quoted.join(", ")} and ${last}`;
  return `You stay in ${list}. The new network gets its own tab in Members.`;
}

/** `via` — the exit the computer keeps using, when it is not direct: the member and the network it is in. */
export function joinedBanner(net: string, via: { peer: string; net: string } | null): string {
  const base = `You joined "${net}".`;
  return via ? `${base} Your exit hasn't changed: sites still open via ${via.peer} (${via.net}).` : base;
}

export const createdBanner = (net: string): string =>
  `You created "${net}". Nobody else is here yet: use Invite to "${net}" to add a member.`;
