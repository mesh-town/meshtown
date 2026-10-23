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
import type { NetBrief } from "../core/api-types-overview.ts";
import { isMulti, netName, slotState } from "../core/slots.ts";
import { targetSlots } from "../core/save-config.ts";
import type { Store } from "../core/store.ts";

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

export function rowsBusy(domains: Array<{ checking?: boolean }> | null | undefined): boolean {
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

/** A network in the site table: its members are the columns of one group. */
export interface SiteNet {
  slot: string;
  label: string;
  selfId: string;
  peers: PeerInfo[];
}

/** One site over all networks that know it. */
export interface MergedDomain {
  name: string;
  checking: boolean;
  byNet: Record<string, DomainInfo>;
}

export interface ColumnGroup {
  slot: string;
  label: string;
  peers: PeerInfo[];
  span: number;
}

/** A mark cell of a row: yours, a member's, or the placeholder of a network without members. */
export interface DomainCell {
  key: string;
  kind: "you" | "peer" | "empty";
  slot: string;
  peer: PeerInfo | null;
  tail: string;
  checked: boolean;
  mark: VerdictView;
}

// One network: the leading slot's state is `store.state`; several: each slot's own state.
function netState(store: Store, slot: string): NodeState | null {
  return isMulti(store) ? slotState(store, slot) : store.state;
}

export function siteNets(store: Store): SiteNet[] {
  const slots = isMulti(store) ? targetSlots(store) : [store.lead];
  const nets: SiteNet[] = [];
  for (const slot of slots) {
    const state = netState(store, slot);
    if (state) nets.push({ slot, label: netName(store.nets, slot), selfId: state.node.id, peers: otherPeers(state) });
  }
  return nets;
}

export function mergedDomains(nets: SiteNet[], store: Store): MergedDomain[] {
  const merged = new Map<string, MergedDomain>();
  for (const net of nets) {
    for (const d of netState(store, net.slot)?.domains || []) {
      const row = merged.get(d.name) || { name: d.name, checking: false, byNet: {} };
      row.byNet[net.slot] = d;
      row.checking = row.checking || d.checking === true;
      merged.set(d.name, row);
    }
  }
  return [...merged.values()];
}

export function columnGroups(nets: SiteNet[]): ColumnGroup[] {
  return nets.map((n) => ({ slot: n.slot, label: n.label, peers: n.peers, span: Math.max(1, n.peers.length) }));
}

// Columns in the table: site, "For you", the groups, actions.
export function tableColspan(groups: ColumnGroup[]): number {
  return 3 + groups.reduce((sum, g) => sum + g.span, 0);
}

// The slot whose exit the "Exit via …" button sets: the one that carries the proxy, otherwise the first.
export function proxySlot(nets: SiteNet[], briefs: NetBrief[]): string {
  const held = nets.find((n) => briefs.some((b) => b.id === n.slot && b.proxy));
  return (held || nets[0])?.slot ?? "main";
}

// "For you" does not depend on the network: the first network that has a verdict for you.
function youCell(d: MergedDomain, nets: SiteNet[]): DomainCell {
  const net = nets.find((n) => d.byNet[n.slot]?.checks?.[n.selfId] != null);
  const check = net ? d.byNet[net.slot].checks[net.selfId] : undefined;
  return { key: "you", kind: "you", slot: net?.slot ?? "", peer: null, tail: "", checked: check !== undefined, mark: verdictView(check) };
}

const NO_ROW_MARK: VerdictView = { cls: "badge-none", text: NONE_TEXT, title: "" };

function netCells(d: MergedDomain, net: SiteNet, tail: string): DomainCell[] {
  const row = d.byNet[net.slot];
  if (net.peers.length === 0) return [{ key: net.slot, kind: "empty", slot: net.slot, peer: null, tail, checked: true, mark: NO_ROW_MARK }];
  return net.peers.map((peer) => {
    const check = row ? (row.checks || {})[peer.id] : undefined;
    return {
      key: net.slot + ":" + peer.id, kind: "peer", slot: net.slot, peer, tail,
      checked: !row || check !== undefined, mark: row ? verdictView(check) : NO_ROW_MARK,
    };
  });
}

export function domainCells(d: MergedDomain, nets: SiteNet[]): DomainCell[] {
  const tail = (n: SiteNet) => (nets.length >= 2 ? n.label : "");
  return [youCell(d, nets), ...nets.flatMap((n) => netCells(d, n, tail(n)))];
}

export function domainExit(d: MergedDomain, nets: SiteNet[], proxy: string, chosen: string[]): ExitCell {
  const row = d.byNet[proxy];
  const net = nets.find((n) => n.slot === proxy);
  return row && net ? exitCell(row, net.peers, net.selfId, chosen) : { kind: "none" };
}
