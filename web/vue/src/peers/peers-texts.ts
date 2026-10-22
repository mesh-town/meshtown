// Texts of the "Members" section: connection, the old-version mark, action captions.

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
