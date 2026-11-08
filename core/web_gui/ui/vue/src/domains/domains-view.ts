/* The "Site checks" section: pure table logic. A row is a site,
 * with "For you" marks and a mark per member, and actions. */

import { agoText, failText } from "../common/words.ts";
import {
  DOMAINS_ACTION_ADD, DOMAINS_ACTION_CHECK_ALL, DOMAINS_ACTION_RECHECK, DOMAINS_ACTION_REMOVE, DOMAINS_ADD_HINT,
  DOMAINS_CHECKING_ALL, DOMAINS_CHECK_ALL, DOMAINS_CHECK_ALL_CONFLICT, DOMAINS_NOTE_ANTIBOT, DOMAINS_NOTE_DNS_MISMATCH,
  DOMAINS_NOTE_DOWN, DOMAINS_NOTE_NETWORK_BLOCKED, DOMAINS_NOTE_OK, DOMAINS_NOTE_SITE_GEOBLOCK, DOMAINS_NOT_CHECKED,
  DOMAINS_UNKNOWN, DOMAINS_VERDICT_ANTIBOT, DOMAINS_VERDICT_DNS_MISMATCH, DOMAINS_VERDICT_DOWN,
  DOMAINS_NOTE_HERE, DOMAINS_TITLE_HERE, DOMAINS_VERDICT_HERE,
  DOMAINS_VERDICT_NETWORK_BLOCKED, DOMAINS_VERDICT_OK, DOMAINS_VERDICT_SITE_GEOBLOCK, domainsCheckedAgoText, domainsUnknownCodeText,
} from "./domains-texts.ts";
import type { CheckInfo, ChosenExit, DomainInfo, NodeState, PeerInfo, Verdict } from "../core/api-types.ts";
import type { NetBrief } from "../core/api-types-overview.ts";
import { isMulti, netName, slotState } from "../core/slots.ts";
import { targetSlots } from "../core/save-config.ts";
import type { Store } from "../core/store.ts";

/** A table cell mark. */
interface VerdictView {
  cls: string;
  text: string;
  title: string;
}


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

const NONE_TEXT = "—";
const UNKNOWN_TEXT = DOMAINS_UNKNOWN;

// Legend under the table: a badge (the text is VERDICT_TEXT verbatim) and its explanation.
export const LEGEND: Array<{ cls: string; text: string; note: string }> = ([
  // verdict: null — the "not checked yet" row
  { verdict: "ok", note: DOMAINS_NOTE_OK },
  { verdict: "dns_mismatch", note: DOMAINS_NOTE_DNS_MISMATCH },
  { verdict: "network_blocked", note: DOMAINS_NOTE_NETWORK_BLOCKED },
  { verdict: "site_geoblock", note: DOMAINS_NOTE_SITE_GEOBLOCK },
  { verdict: "antibot", note: DOMAINS_NOTE_ANTIBOT },
  { verdict: "here", note: DOMAINS_NOTE_HERE },
  { verdict: "down", note: DOMAINS_NOTE_DOWN },
  { verdict: null, note: DOMAINS_NOT_CHECKED },
] as Array<{ verdict: Verdict | "here" | null; note: string }>).map((e) => ({
  cls: (e.verdict === "here" ? VERDICT_BADGE.network_blocked : e.verdict && VERDICT_BADGE[e.verdict]) || "badge-none",
  text: (e.verdict === "here" ? DOMAINS_VERDICT_HERE : e.verdict && VERDICT_TEXT[e.verdict]) || NONE_TEXT,
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

export function rowsBusy(domains: Array<{ checking?: boolean }> | null | undefined): boolean {
  return (domains || []).some((d) => d.checking === true);
}

export function checkAllLabel(disabled: boolean): string {
  return disabled ? DOMAINS_CHECKING_ALL : DOMAINS_CHECK_ALL;
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

/** A mark cell of a row: yours, a member's, or the placeholder of a network without members. */
interface DomainCell {
  key: string;
  kind: "you" | "peer" | "empty";
  slot: string;
  peer: PeerInfo | null;
  tail: string;
  checked: boolean;
  check: CheckInfo | null | undefined;
  mark: VerdictView;
}

// One network: the leading slot's state is `store.state`; several: each slot's own state.
export function netState(store: Store, slot: string): NodeState | null {
  return isMulti(store) ? slotState(store, slot) : store.state;
}

export function siteNets(store: Store): SiteNet[] {
  const slots = isMulti(store) ? targetSlots(store) : [store.lead];
  const nets: SiteNet[] = [];
  for (const slot of slots) {
    const state = netState(store, slot);
    if (state) nets.push({ slot, label: netName(store.nets, slot), selfId: state.node.id, peers: otherPeers(state).filter((p) => p.exit === true) });
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

// The slot whose exit the "Exit via …" button sets: the one that carries the proxy, otherwise the first.
export function proxySlot(nets: SiteNet[], briefs: NetBrief[]): string {
  const held = nets.find((n) => briefs.some((b) => b.id === n.slot && b.proxy));
  return (held || nets[0])?.slot ?? "main";
}

const HERE_MARK: VerdictView = { cls: VERDICT_BADGE.network_blocked, text: DOMAINS_VERDICT_HERE, title: DOMAINS_TITLE_HERE };

// Some member (not you) of any network has "ok" for the site.
function memberOk(d: MergedDomain, nets: SiteNet[]): boolean {
  return nets.some((n) => n.peers.some((p) => d.byNet[n.slot]?.checks?.[p.id]?.verdict === "ok"));
}

// "For you" does not depend on the network: the first network that has a verdict for you.
export function ownCheck(d: MergedDomain, nets: SiteNet[]): { slot: string; check: CheckInfo | undefined } {
  const net = nets.find((n) => d.byNet[n.slot]?.checks?.[n.selfId] != null);
  return { slot: net?.slot ?? "", check: net ? d.byNet[net.slot].checks[net.selfId] ?? undefined : undefined };
}

// The direct check with the "doesn't open from here" rule: down for you while some member opens it.
export function directMark(d: MergedDomain, nets: SiteNet[]): VerdictView {
  const { check } = ownCheck(d, nets);
  return check?.verdict === "down" && memberOk(d, nets) ? HERE_MARK : verdictView(check);
}

function youCell(d: MergedDomain, nets: SiteNet[]): DomainCell {
  const { slot, check } = ownCheck(d, nets);
  return { key: "you", kind: "you", slot, peer: null, tail: "", checked: check !== undefined, check, mark: directMark(d, nets) };
}

const NO_ROW_MARK: VerdictView = { cls: "badge-none", text: NONE_TEXT, title: "" };

function netCells(d: MergedDomain, net: SiteNet, tail: string, multi: boolean): DomainCell[] {
  const row = d.byNet[net.slot];
  if (net.peers.length === 0 && !multi) return [];
  if (net.peers.length === 0) return [{ key: net.slot, kind: "empty", slot: net.slot, peer: null, tail, checked: true, check: undefined, mark: NO_ROW_MARK }];
  return net.peers.map((peer) => {
    const check = row ? (row.checks || {})[peer.id] : undefined;
    return {
      key: net.slot + ":" + peer.id, kind: "peer", slot: net.slot, peer, tail,
      checked: !row || check !== undefined, check, mark: row ? verdictView(check) : NO_ROW_MARK,
    };
  });
}

export function domainCells(d: MergedDomain, nets: SiteNet[]): DomainCell[] {
  const tail = (n: SiteNet) => (nets.length >= 2 ? n.label : "");
  return [youCell(d, nets), ...nets.flatMap((n) => netCells(d, n, tail(n), nets.length >= 2))];
}

/** The exits of the network that carries the proxy, as the status line reads them. */
export function routeExits(store: Store, slot: string): ChosenExit[] {
  return netState(store, slot)?.route?.exits || [];
}
