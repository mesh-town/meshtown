// Texts and rows of the exit-owner block: how many connections are open, why members are turned away, who it hit.
import type { ExitLoad, OverloadReason, PeerInfo } from "../core/api-types.ts";

export const EXIT_LIMIT_LABEL = "Connection limit";
export const EXIT_LIMIT_INVALID = "Enter a whole number from 0 to 100000";
export const EXIT_LIMIT_MAX = 100000;
export const EXIT_TURNED_AWAY_TITLE = "Turned away in the last 10 minutes";

export const exitStreamsLine = (load: Pick<ExitLoad, "streams" | "limit">): string =>
  `Connections open through this computer: ${load.streams} of ${load.limit}`;

export function exitLimitHint(limitAuto: number): string {
  return `The most connections members can have open through this computer at once. 0 — chosen from this computer's memory (now ${limitAuto}). Applies at once, to all your networks.`;
}

/** The warning for the reason the exit gives; `null` — nothing to warn about (no overload, or the reason is not the owner's). */
export function exitWarningText(reason: OverloadReason | null | undefined, limit: number): string | null {
  const head = "This computer is turning members away: ";
  if (reason === "limit") return `${head}all ${limit} connections are in use. Raise the connection limit below.`;
  if (reason === "share") return `${head}a member has used up its share of the ${limit} connections. Raise the connection limit below.`;
  if (reason === "memory") return `${head}it is low on memory. Raising the limit will not help — fewer members or a machine with more memory will.`;
  return null;
}

export const exitOtherNetworksRow = (n: number): string => `Members of your other networks — ${n}`;

/** Table rows: only members with refusals, most refused first; the name comes from `peers[]` by the full id. */
export function exitTurnedAwayRows(load: ExitLoad, peers: PeerInfo[] | null | undefined): string[] {
  const nameOf = (id: string): string => {
    const p = (peers ?? []).find((x) => x.id_full === id);
    return p?.name || id.slice(0, 8);
  };
  const rows = [...(load.members ?? [])]
    .filter((m) => m.refused > 0)
    .sort((a, b) => b.refused - a.refused)
    .map((m) => `${nameOf(m.id_full)} — ${m.refused} of ${m.admitted + m.refused} connections`);
  if (load.elsewhere_refused > 0) rows.push(exitOtherNetworksRow(load.elsewhere_refused));
  return rows;
}

/** Parses the field text: a whole number from 0 to 100000, otherwise `null`. */
export function parseExitLimit(text: string): number | null {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return null;
  const n = Number(t);
  return n <= EXIT_LIMIT_MAX ? n : null;
}
