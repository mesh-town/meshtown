/* Texts of the "Diagnostics" section: status lights, events, the summary for help, details for specialists, the network key. */

import { EXIT_DOWN_TAIL, NET_NAME } from "../common/words.ts";
import { plural } from "../common/locale.ts";

// --- Shared by components ---
export const DIAG_STORE_MISSING = "STORE_KEY not provided";
export const DIAG_LOADING = "Loading…";
export const DIAG_CHECKS_LABEL = "Checks";
export const DIAG_STATE_TITLE = "Status";
export const DIAG_EVENTS_TITLE = "Recent events";
export const DIAG_EVENTS_EMPTY = "No events yet";
export const DIAG_FULL_LOG_LINK = "Full log →";
export const DIAG_SUMMARY_BTN = "Copy summary for help";
export const DIAG_SUMMARY_LABEL = "Summary for help";

// --- Technical details ---
export const DIAG_SPECIALIST_TITLE = "Technical details";
export const DIAG_CONNECTION_TITLE = "Connection status";
export const DIAG_MANUAL_TITLE = "Connect to a member manually";
export const DIAG_SELF_ADDRESSES_LABEL = "Your addresses (send them to the member)";
export const DIAG_COPY_BTN = "Copy";
export const DIAG_PEER_ADDRESSES_LABEL = "The member's addresses";
export const DIAG_PEER_ADDRESSES_PLACEHOLDER = "Paste the member's addresses";
export const DIAG_ADD_BTN = "Add";
export const DIAG_MANUAL_HINT = "Needed when this computer doesn't find other members through the address directory — for example, because of strict protection on the home network.";
export const DIAG_ABOUT_TITLE = "About";

// --- Network key ---
export const DIAG_KEY_HIDE = "Hide";
export const DIAG_KEY_SHOW = "Show";
export const DIAG_KEY_TITLE = NET_NAME + " network key";
export const DIAG_KEY_HINT_HEAD = "The key gives access to your ";
export const DIAG_KEY_HINT_TAIL = " network, just like an invite: don't send it to a group chat. To invite someone, use \"Invite a member\" on the \"Home\" tab.";

// --- Actions (diag-actions) ---
export const DIAG_SUMMARY_COPIED = "Summary copied";
export const DIAG_CLIPBOARD_FALLBACK = "Clipboard unavailable — select the summary below and copy it manually.";
export const DIAG_ADDRESSES_COPIED = "Copied";
export const DIAG_ADDRESSES_FALLBACK = "Clipboard unavailable — select the addresses and copy them manually.";
export const DIAG_NO_LOG_DATA = "no data";
export const DIAG_NO_ANSWER = "no answer";
export const DIAG_PEER_ADDR_EMPTY = "Paste the member's addresses — one per line";
export const DIAG_KEY_MISSING = NET_NAME + " didn't return the network key — try again";
export const DIAG_KEY_STALE = "The network changed — click \"Show\" again.";
export const DIAG_NO_NETWORK_KEY = "You haven't joined the network — there is no key yet";
export const DIAG_FAIL_ADD_ADDRESSES = "add the addresses";
export const DIAG_FAIL_GET_KEY = "get the network key";

// --- Status lights (diag-checks) ---
export const DIAG_NO_NETWORK = "You haven't joined the network — paste an invite or create a network on \"Home\"";
export const DIAG_NO_DATA = " — no data";
export const DIAG_NODE_STARTING = NET_NAME + " is starting…";
export const DIAG_NODE_STOPPING = NET_NAME + " is stopping…";
export const DIAG_NODE_RUNNING = NET_NAME + " is running";
export const DIAG_DIRECTORY_LABEL = "Address directory";
export const DIAG_DIRECTORY_OK = "The address directory is connected — members can find you";
export const DIAG_DIRECTORY_WARN = "The address directory is not connected — members may not find you";
export const DIAG_PEERS_LABEL = "Members";
export const DIAG_PEERS_ALONE = "Nobody else is in the network yet — invite a member on \"Home\"";
export const DIAG_PEERS_NONE_ONLINE = "No members are online";
export const DIAG_EXIT_DEFAULT_NAME = "The chosen member";
export const DIAG_EXIT_LABEL = "How sites open";
export const DIAG_EXIT_VIA_DEFAULT = "the chosen member";
export const DIAG_PROXY_LABEL = "Browser proxy";
export const DIAG_PROXY_OFF = "The browser proxy is off";
const DIAG_ROUTE_WONT_OPEN_PREFIX = "In the browser, sites won’t open through ";

export function diagNetNoConnectionText(net: string): string {
  return net + ": no connection to the " + NET_NAME + " network";
}

export function diagNetAloneText(net: string): string {
  return net + ": nobody else is here yet";
}

export function diagNetDirectoryWarnText(net: string): string {
  return net + ": the address directory is not connected — members may not find you";
}

export function diagNetPeersText(net: string, online: number, total: number): string {
  return net + ": " + online + " of " + total + " " + plural(total, "member", "members") + " online";
}

export function diagRouteWontOpenText(peer: string): string {
  return DIAG_ROUTE_WONT_OPEN_PREFIX + peer;
}

export function diagUptimeText(hours: number, minutes: number): string {
  return hours + " h " + minutes + " min";
}

export function diagMibText(mib: string): string {
  return mib + " MiB";
}

export function diagRunningMinutesText(minutes: number): string {
  return NET_NAME + " has been running for " + minutes + " min";
}

export function diagRunningUptimeText(uptime: string): string {
  return NET_NAME + " has been running for " + uptime;
}

export const DIAG_NETWORK_JOINED_NO_LABEL = "You're in a " + NET_NAME + " network";

export function diagNetworkJoinedText(name: string): string {
  return "You're in the network " + name;
}

export function diagPeersOnlineText(online: number, total: number): string {
  const noun = plural(total, "member", "members");
  return online + " of " + total + " " + noun + " online";
}

export function diagExitUnavailableText(name: string): string {
  return name + " is offline — " + EXIT_DOWN_TAIL;
}

export function diagProxyOnText(listen: string | null): string {
  return "The browser proxy is running on " + listen;
}

// --- Details (diag-details) ---
export const DIAG_ROW_PORT = "port for connections";
export const DIAG_ROW_DIRECTORY = "address directory";
export const DIAG_ROW_RECORD = "member record";
export const DIAG_DHT_NONE = "no entry point";
export const DIAG_DHT_CONNECTED = "connected";
export const DIAG_RECORD_NONE = "not published";
export const DIAG_RECORD_PUBLISHED = "published";

export function diagDhtRefreshedText(sec: number): string {
  return "; record refreshed " + sec + " s ago";
}

export function diagDhtNodesText(nodes: number): string {
  return ", directory nodes: " + nodes;
}

export function diagRecordTtlText(ttlSec: number): string {
  return "published, valid for another " + ttlSec + " s";
}

export function diagAboutHeadText(addr: string, version: string): string {
  return NET_NAME + " on this computer · " + addr + " · version " + version;
}

export function diagAboutMemoryText(mem: string, uptime: string): string {
  return "Memory " + mem + " · uptime " + uptime;
}

export function diagPeerAddedText(added: number, rejected: number, tail: string): string {
  return "Addresses added: " + added + ", rejected: " + rejected + tail;
}

// --- Summary and cleanup ---
export const DIAG_LOG_UNAVAILABLE_PREFIX = "log unavailable: ";
export const DIAG_SUMMARY_LOG_HEADER = "Latest log lines:";
export const DIAG_HIDDEN_TOKEN = "token=‹hidden›";
export const DIAG_HIDDEN_ADDR = "‹address›";
