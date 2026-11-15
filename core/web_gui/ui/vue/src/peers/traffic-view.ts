// The traffic line of a member row: bytes through the exit since the node started, in both directions.

import type { PeerInfo } from "../core/api-types.ts";
import { PEERS_TRAFFIC_VIA_THEM, PEERS_TRAFFIC_VIA_YOU } from "./peers-texts.ts";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Decimal size: below 1000 — "N B", then one digit after the point under 10, otherwise a whole number. */
export function bytesText(n: number): string {
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return Math.round(value) + " B";
  const shown = value < 10 ? value.toFixed(1) : String(Math.round(value));
  return shown + " " + UNITS[unit];
}

/** Non-zero parts joined with " · "; own row and both zeros — an empty string. */
export function peerTrafficText(p: PeerInfo): string {
  if (p.self) return "";
  const parts: string[] = [];
  if (p.through_peer_bytes) parts.push(PEERS_TRAFFIC_VIA_THEM + bytesText(p.through_peer_bytes));
  if (p.through_us_bytes) parts.push(PEERS_TRAFFIC_VIA_YOU + bytesText(p.through_us_bytes));
  return parts.join(" · ");
}
