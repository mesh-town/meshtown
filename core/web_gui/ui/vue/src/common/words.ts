import type { Absence } from "../core/store.ts";

/** A member for a label: `id` and an optional `name` are enough (`PeerInfo`, `ChosenExit`, etc.). */
export interface NamedPeer {
  id: string;
  name?: string | null;
}

// An error for a text: `ApiError`, `Error` or anything from `catch` — only `message` and `body.message` are read.
interface ErrorWithBody {
  message?: unknown;
  body?: { message?: unknown } | null;
}

/* Page words shared by the sections: the network name in texts and the label of a
 * node without a name. A copy of the classic `00-core.js::NET_NAME` and `peerName` (it was in the removed
 * `10-domains.js`) — the duplicates are reconciled when the classic code is removed (the classic copies of `peerLabel`, `anonymousPeerLabel` are in
 * `14-home-exit-rows.js` and will go with the last classic consumer). */

export const NET_NAME = "Mesh Town";

// A note on a field set by a node startup flag (Home and Settings).
export const CLI_FIXED_NOTE = "set at node startup — can't be changed here";

export function anonymousPeerLabel(p: NamedPeer): string {
  return "member " + p.id;
}

// The node name for the first line of the list; without a name — "member <id>".
export function peerName(p: NamedPeer): string {
  return p.name && p.name.trim() ? p.name : anonymousPeerLabel(p);
}

/** The tail of the phrase "the exit is offline": when the exit is unreachable, some browsers go direct. */
export const EXIT_DOWN_TAIL = "sites may not open or may open directly";

const nameKey = (p: NamedPeer): string => (p.name || "").trim().toLowerCase();

// Another member of `all` (by id) with the same name — the id is needed to tell them apart.
function sharesName(p: NamedPeer, all: NamedPeer[]): boolean {
  return all.some((o) => o.id !== p.id && nameKey(o) === nameKey(p));
}

// The name; "name (id)" only when `all` holds another member with the same name; without a name —
// "member <id>" (a column header is a standalone label) or just the id.
export function peerLabel(p: NamedPeer, withAnonymousTag?: boolean, all: NamedPeer[] = []): string {
  if (p.name && p.name.trim()) return sharesName(p, all) ? p.name + " (" + p.id + ")" : p.name;
  return withAnonymousTag ? anonymousPeerLabel(p) : p.id;
}

// ' "name"' for phrases like 'into the network "family"'; no label or an empty one — an empty string, so that
// the text never contains '"null"'.
export function quotedNet(label: unknown): string {
  return typeof label === "string" && label.trim() ? ' "' + label + '"' : "";
}

// The first letter uppercase (a node message at the start of a sentence); empty — "". The program name
// at the start stays as is, as everywhere on the page and in the tray: "Mesh Town is not responding".
export function upperFirst(text: unknown): string {
  if (typeof text !== "string" || !text) return "";
  if (text.startsWith(NET_NAME)) return text;
  return text[0].toUpperCase() + text.slice(1);
}

// The first letter lowercase, only if the second is lowercase too ("Node" → "node", "DNS" stays).
export function lowerFirst(text: string): string {
  if (text.startsWith(NET_NAME)) return text;
  const isWord = text.length > 1 && text[0] !== text[0].toLowerCase() && text[1] === text[1].toLowerCase() && text[1] !== text[1].toUpperCase();
  return isWord ? text[0].toLowerCase() + text.slice(1) : text;
}

// The reason for a failure: the node message from the response body, otherwise the error message; neither — "".
export function errReason(err: unknown): string {
  const e = err as ErrorWithBody | null | undefined;
  return String((e && e.body && e.body.message) || (e && e.message) || "").trim();
}

// "Couldn't {action}: {reason}"; no reason — "Couldn't {action} — {hint}".
export function failText(action: string, err: unknown, hint: string = "try again"): string {
  const reason = errReason(err);
  if (!reason) return "Couldn't " + action + " — " + hint;
  return "Couldn't " + action + ": " + lowerFirst(reason);
}

// "just now" / "N min ago" / "N h ago" / "N d ago"; not a number — "".
export function agoText(sec: unknown): string {
  if (typeof sec !== "number" || !Number.isFinite(sec)) return "";
  if (sec < 60) return "just now";
  if (sec < 3600) return Math.floor(sec / 60) + " min ago";
  if (sec < 86400) return Math.floor(sec / 3600) + " h ago";
  return Math.floor(sec / 86400) + " d ago";
}

// The exit choice label: the single source.
export const EXIT_VIA_HIM = "Use as exit";

export function exitViaText(name: string): string {
  return "Exit via " + name;
}

// Phrases about how sites open — as in the tray: they speak of the browser with the proxy.
export const ROUTE_DIRECT_TEXT = "In the browser, sites open directly";

export function routeViaText(label: string): string {
  return "In the browser, sites open via " + label;
}

// The short phrase for the header: a browser says what sites do, the app names the exit.
export const ROUTE_LINE_DIRECT = "Sites open directly";
export const ROUTE_LINE_APP_DIRECT = "Exit: none, sites open directly";
export const routeLineViaText = (label: string): string => "Sites open via " + label;
export const routeLineAppViaText = (label: string): string => "Exit: " + label;

// The page couldn't reach the node: the program is named by name, as in "{NET_NAME} is stopped".
export const NOT_RESPONDING_TEXT = NET_NAME + " is not responding";

const START_HINT = "Start it: the " + NET_NAME + " icon near the clock → \"Start " + NET_NAME + "\".";

// The "the page gets no snapshots" texts — shared by the header and the Home panel.
export const ABSENCE_TEXT: Record<Absence, string> = {
  token: "A new page link is needed",
  silent: NOT_RESPONDING_TEXT,
  stopped: NET_NAME + " is stopped",
};

export const ABSENCE_HINT: Record<Absence, string> = { token: "", silent: START_HINT, stopped: START_HINT };

// The creator of the network is alone: the header and Home use the same words.
export const ALONE_TEXT = "only you for now";
export const ALONE_INVITE_TEXT = "Click \"Invite a member\" and send the invite — the new member will appear here as soon as they connect.";

// Three network states: the header and Home take their words from here.
export const NET_LOADING_TEXT = "loading network data…";
export const ALONE_INVITE_SHORT = "invite members";

export function offlineOthersText(n: number): string {
  return n === 1 ? "the other member is offline" : "all other members are offline";
}

// The address directory is unavailable after 60 s of running: lead to an invite.
export const NO_CATALOG_SHORT = "members can't find you automatically";
export const NO_CATALOG_INVITE_TEXT = NET_NAME + " can't reach the address directory that helps members find each other. Invite members anyway: the invite contains your address, so they can connect without it.";

export function noCatalogOthersText(n: number): string {
  const first = offlineOthersText(n);
  return first.charAt(0).toUpperCase() + first.slice(1) + ", and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
}
