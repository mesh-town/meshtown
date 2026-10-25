// Texts of the "Members" section: connection, the old-version mark, action captions.

import { forDevice } from "../common/device.ts";

export function peersOlderTitleText(peerVersion: string | null | undefined, ownVersion: string | null | undefined): string {
  return "This member has Mesh Town " + peerVersion + ", you have " + ownVersion + ". Ask them to update Mesh Town.";
}

export const PEERS_OFFLINE = "offline";

export function peersOfflineSeenText(seen: string): string {
  return "offline · last seen " + seen;
}

export const PEERS_ONLINE = "online";
export const PEERS_DIRECT = "direct";
export const PEERS_RELAY = "via relay server";

export function peersRttText(ms: number): string {
  return ms + " ms";
}

export const PEERS_SELF_EXIT_ON = "Others can open sites through you";
export const PEERS_SELF_EXIT_OFF = "Others can't open sites through you";

export const PEERS_NO_STORE = "PeersSection: store not provided (STORE_KEY)";
export const PEERS_LOADING = "Loading…";
export const PEERS_YOU_PREFIX = "You: ";
export const PEERS_OLDER = "older version";
export const PEERS_EDIT = "Rename";
export const PEERS_CHOSEN_OK = "✓ Your exit";
export const PEERS_CHOSEN_DOWN = "Your exit — unavailable right now";
export const PEERS_REFUSED = "Not available as an exit";

// Several networks: the tab row, the tab header, the hint.
export const PEERS_INVITE_ONE = "Invite a member";
export const PEERS_INVITE_COPIED = "Invite copied. It works once, for 24 hours — send it to one person in a private message.";
export const PEERS_TAB_ADD = "Add network";
export const PEERS_TAB_ADD_FULL = "Mesh Town can be in up to 8 networks at once";
export const PEERS_TAB_ADD_MARK = "+";
export const PEERS_TAB_JOINING = "Joining the network…";
export const PEERS_TAB_JOINING_SUB = "· joining…";
export const PEERS_ADMIN_BADGE = "Admin";
export const PEERS_TAB_DELETED_SUB = "· deleted";
export const PEERS_TAB_REFUSED_SUB = "· refused";
export const PEERS_TAB_REMOVED_SUB = "· removed";
export const PEERS_NEW_NET = "New network";
const PEERS_TAB_NO_CONN = "· no connection";
export const PEERS_NETS_HINT_OK = "OK";
export const PEERS_NETS_HINT_KEY = "meshtown.netsHintSeen";

export function peersInviteTo(net: string): string {
  return `Invite to "${net}"`;
}

// "just you" · "you and 1 other, online" · "you and 2 others, both online" · "you and 5 others, all online" · "you and 5 others, 3 online".
export function peersWhoText(online: number, total: number): string {
  if (total <= 0) return "just you";
  const others = `you and ${total} ${total === 1 ? "other" : "others"}, `;
  if (online !== total) return `${others}${online} online`;
  if (total === 1) return others + "online";
  return others + (total === 2 ? "both" : "all") + " online";
}

export function peersNetLine(net: string, who: string): string {
  return `Network "${net}" · ${who}`;
}

export function peersTabSub(online: number, down: boolean): string {
  return down ? PEERS_TAB_NO_CONN : `· ${online} online`;
}

export const peersManagedTabSub = (online: number): string => `· managed · ${online} online`;

export function peersTabTitle(net: string, online: number, total: number, down: boolean): string {
  return down ? `${net} — no connection` : `${net} — ${online} of ${total} members online`;
}

export function peersVotesTitle(n: number): string {
  return `${n} to confirm`;
}

export function peersNetsHint(n: number): string {
  return `You’re now in ${n} networks. Each network has its own tab here. Your name, "Let members open sites" and "Leave network" are now in each network’s tab.`;
}

/** How long the refusal has to stand before the plate shows: right after joining "0 online" is normal. */
export const ADMISSION_REFUSED_DELAY_MS = 15_000;

export const ADMISSION_REFUSED_TITLE = (net: string): string => forDevice("This computer can't connect to ") + `"${net}"`;
const NO_ONE_ELSE = (net: string): string => `and no other member of "${net}" is online.`;
export const ADMISSION_REFUSED_ONE = (name: string, net: string): string => `${name} refuses to let it in, ${NO_ONE_ELSE(net)}`;
export const ADMISSION_REFUSED_MANY = (list: string, net: string): string => `${list} refuse to let it in, ${NO_ONE_ELSE(net)}`;
export const ADMISSION_REFUSED_NONE = (net: string): string => `Members of "${net}" refuse to let it in, and no other member is online.`;
const UNTIL_THEN = (net: string): string => ` Until then, sites won't open through "${net}".`;
/** What to do: a Managed network lets in through an admin; in a Peer network the refusing members are asked. */
export function admissionRefusedAsk(names: string[], net: string, managed: boolean): string {
  if (managed) return `Ask an admin of "${net}" ` + forDevice("to let this computer in.") + UNTIL_THEN(net);
  const who = names.length === 0 ? "a member" : names.length === 1 ? names[0] : "one of them";
  return `Ask ${who} ` + forDevice("to let this computer in.") + UNTIL_THEN(net);
}

// 'a' · 'a and b' · 'a, b and c' · 'a, b, c and 2 more'
export function admissionRefusedList(names: string[]): string {
  const shown = names.slice(0, 3);
  const more = names.length - shown.length;
  if (more > 0) return `${shown.join(", ")} and ${more} more`;
  const last = shown.pop();
  return shown.length === 0 ? (last ?? "") : `${shown.join(", ")} and ${last}`;
}
