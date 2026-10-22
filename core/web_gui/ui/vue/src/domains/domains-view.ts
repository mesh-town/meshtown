/* The "Site checks" section: pure table logic (a port of `js/10-domains.js`). A row is a site,
 * with "For you" marks and a mark per member, and actions. */

import { agoText, failText } from "../common/words.ts";
import {
  DOMAINS_ACTION_ADD, DOMAINS_ACTION_CHECK_ALL, DOMAINS_ACTION_RECHECK, DOMAINS_ACTION_REMOVE, DOMAINS_ADD_HINT,
  DOMAINS_CHECKING_ALL, DOMAINS_CHECK_ALL, DOMAINS_CHECK_ALL_CONFLICT, DOMAINS_NOTE_ANTIBOT, DOMAINS_NOTE_DNS_MISMATCH,
  DOMAINS_NOTE_DOWN, DOMAINS_NOTE_NETWORK_BLOCKED, DOMAINS_NOTE_OK, DOMAINS_NOTE_SITE_GEOBLOCK, DOMAINS_NOT_CHECKED,
  DOMAINS_PEER_FALLBACK, DOMAINS_UNKNOWN, DOMAINS_VERDICT_ANTIBOT, DOMAINS_VERDICT_DNS_MISMATCH, DOMAINS_VERDICT_DOWN,
  DOMAINS_VERDICT_NETWORK_BLOCKED, DOMAINS_VERDICT_OK, DOMAINS_VERDICT_SITE_GEOBLOCK, domainsCheckedAgoText, domainsUnknownCodeText,
} from "./domains-texts.ts";
import type { CheckInfo, DomainInfo, NodeState, PeerInfo, Verdict } from "../core/api-types.ts";

/** A table cell mark. */
export interface VerdictView {
  cls: string;
  text: string;
  title: string;
}

export type ExitCell = { kind: "none" } | { kind: "chosen" | "button"; peer: PeerInfo };

const isVerdict = (v: string): v is Verdict => Object.hasOwn(VERDICT_TEXT, v);

export const VERDICT_BADGE: Record<Verdict, string> = {
  ok: "badge-ok",
  dns_mismatch: "badge-dns",
  network_blocked: "badge-cut",
  site_geoblock: "badge-geo",
  antibot: "badge-bot",
  down: "badge-down",
};

export const VERDICT_TEXT: Record<Verdict, string> = {
  ok: DOMAINS_VERDICT_OK,
  dns_mismatch: DOMAINS_VERDICT_DNS_MISMATCH,
  network_blocked: DOMAINS_VERDICT_NETWORK_BLOCKED,
  site_geoblock: DOMAINS_VERDICT_SITE_GEOBLOCK,
  antibot: DOMAINS_VERDICT_ANTIBOT,
  down: DOMAINS_VERDICT_DOWN,
};

// The scale is `agoText`: deliberately coarse, the mark's title must not change on every poll.

export function checkTitle(check: CheckInfo | null | undefined): string {
  if (!check) return DOMAINS_NOT_CHECKED;
  const age = agoText(check.age_sec);
  return [check.reason, age && domainsCheckedAgoText(age)].filter(Boolean).join(" · ");
}

export const NONE_TEXT = "—";
export const UNKNOWN_TEXT = DOMAINS_UNKNOWN;

// Legend under the table: a badge (the text is VERDICT_TEXT verbatim) and its explanation.
export const LEGEND: Array<{ cls: string; text: string; note: string }> = ([
  // verdict: null — the "not checked yet" row
  { verdict: "ok", note: DOMAINS_NOTE_OK },
  { verdict: "dns_mismatch", note: DOMAINS_NOTE_DNS_MISMATCH },
  { verdict: "network_blocked", note: DOMAINS_NOTE_NETWORK_BLOCKED },
  { verdict: "site_geoblock", note: DOMAINS_NOTE_SITE_GEOBLOCK },
  { verdict: "antibot", note: DOMAINS_NOTE_ANTIBOT },
  { verdict: "down", note: DOMAINS_NOTE_DOWN },
  { verdict: null, note: DOMAINS_NOT_CHECKED },
] as Array<{ verdict: Verdict | null; note: string }>).map((e) => ({
  cls: (e.verdict && VERDICT_BADGE[e.verdict]) || "badge-none",
  text: (e.verdict && VERDICT_TEXT[e.verdict]) || NONE_TEXT,
  note: e.note,
}));

// A cell mark: class, text and hint; no record — "—"; an unknown code — "result unknown".
export function verdictView(check: CheckInfo | null | undefined): VerdictView {
  const verdict: string | null = check ? check.verdict : null;
  if (verdict && !isVerdict(verdict)) {
    return { cls: "badge-none", text: UNKNOWN_TEXT, title: domainsUnknownCodeText(verdict) };
  }
  const known = verdict && isVerdict(verdict) ? verdict : null;
  return {
    cls: (known && VERDICT_BADGE[known]) || "badge-none",
    text: (known && VERDICT_TEXT[known]) || NONE_TEXT,
    title: checkTitle(check),
  };
}

export function otherPeers(state: NodeState | null | undefined): PeerInfo[] {
  return ((state && state.peers) || []).filter((p) => !p.self);
}

// Header of a member column: the name on top, the id on the line below (the column is narrower); without a name — "member".
export function columnHead(p: PeerInfo): { name: string; id: string } {
  return { name: (p.name && p.name.trim()) || DOMAINS_PEER_FALLBACK, id: p.id };
}

// Columns of mark cells: first you, then the members in snapshot order.
export function checkColumns(state: NodeState, others: PeerInfo[]): string[] {
  return [state.node.id].concat(others.map((p) => p.id));
}

// The site is blocked for you (a record exists and is not ok), and opens at a member who is an exit and online;
// the first in snapshot order.
export function domainExitPeer(d: DomainInfo, others: PeerInfo[], selfId: string): PeerInfo | null {
  const checks = d.checks || {};
  const own = checks[selfId];
  if (!own || own.verdict === "ok") return null;
  return (
    others.find((p) => {
      const c = checks[p.id];
      return p.exit === true && p.online && c && c.verdict === "ok";
    }) || null
  );
}

// What to show in the actions cell: nothing / the mark of the chosen one / the "Exit via …" button.
export function exitCell(d: DomainInfo, others: PeerInfo[], selfId: string, chosen: string[]): ExitCell {
  const peer = domainExitPeer(d, others, selfId);
  if (!peer) return { kind: "none" };
  return { kind: chosen[0] === peer.id_full ? "chosen" : "button", peer };
}

export function rowsBusy(domains: DomainInfo[] | null | undefined): boolean {
  return (domains || []).some((d) => d.checking === true);
}

export function checkAllLabel(disabled: boolean): string {
  return disabled ? DOMAINS_CHECKING_ALL : DOMAINS_CHECK_ALL;
}

// Columns in the table: site, "For you", members, actions.
export function emptyColspan(others: PeerInfo[]): number {
  return others.length + 3;
}

export const CHECK_ALL_CONFLICT = DOMAINS_CHECK_ALL_CONFLICT;
export const ADD_HINT = DOMAINS_ADD_HINT;

export const addFailedText = (err: unknown): string => failText(DOMAINS_ACTION_ADD, err, ADD_HINT);
export const recheckFailedText = (name: string, err: unknown): string => failText(DOMAINS_ACTION_RECHECK + name, err);
export const removeFailedText = (name: string, err: unknown): string => failText(DOMAINS_ACTION_REMOVE + name, err);
export const checkAllFailedText = (err: unknown): string => failText(DOMAINS_ACTION_CHECK_ALL, err);
