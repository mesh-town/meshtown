/* Exit choice on "Home" — pure functions without Vue and DOM (a port of `14-home-exit-choice.js`,
 * `14-home-exit-rows.js`, `14-home-exit.js`): the candidates of the "Exit via…" list,
 * row labels, the "Check a site" result. */

import type { ChosenExit, CheckInfo, DomainInfo, NodeState, PathKind, PeerInfo } from "../core/api-types.ts";
import { peerLabel } from "../common/words.ts";
import { BADGE_BLOCKED } from "../members/members-texts.ts";
import { HOME_LINK_DIRECT, HOME_LINK_OFFLINE, HOME_LINK_ONLINE, HOME_LINK_RELAY, rttMsText } from "./home-flow-texts.ts";

/** A row of the "Exit via…" list. */
export interface ExitCandidate {
  idFull: string;
  label: string;
  country: string | null;
  online: boolean;
  active: boolean;
  path: PathKind | null;
  rttMs: number | null;
  checked: boolean;
  blocked?: boolean;
}

export interface ExitItem {
  idFull: string;
  label: string;
}

export type FindExitView =
  | { kind: "none" | "checking" | "empty"; domain: string; items: never[] }
  | { kind: "ok"; domain: string; items: ExitItem[] };

type FindKind = FindExitView["kind"];
type Checks = Record<string, CheckInfo | null>;

function indexBy<T>(list: T[], key: keyof T): Map<unknown, T> {
  return new Map(list.map((item) => [item[key], item]));
}

function uniq<T>(list: T[]): T[] {
  return [...new Set(list)];
}

// The country code as text, without a flag emoji.
export function countryTag(item: { country: string | null }): string {
  return item.country ? " · " + countryName(item.country) : "";
}

// "FR" → "France"; no `Intl.DisplayNames`, an unknown code or a result equal to the code — the code as is.
function countryName(code: string): string {
  try {
    return new Intl.DisplayNames(["en"], { type: "region" }).of(code) || code;
  } catch {
    return code;
  }
}

// On the right, small: connection, path (direct / via relay server), latency.
export function exitMetaText(item: Pick<ExitCandidate, "online" | "path" | "rttMs" | "blocked">): string {
  if (item.blocked) return BADGE_BLOCKED;
  return item.online ? onlineMeta(item) : HOME_LINK_OFFLINE;
}

const PATH_WORDS: Record<PathKind, string> = { direct: HOME_LINK_DIRECT, relay: HOME_LINK_RELAY };

function onlineMeta(item: Pick<ExitCandidate, "path" | "rttMs">): string {
  const path = item.path ? PATH_WORDS[item.path] : undefined;
  const pathPart = path ? [path] : [];
  const rtt = item.rttMs != null ? [rttMsText(item.rttMs)] : [];
  return [HOME_LINK_ONLINE, ...pathPart, ...rtt].join(" · ");
}

// A list row: for an unnamed member — "member <id>" (the label stands on its own).
function exitCandidate(idFull: string, peer: PeerInfo | undefined, stateEntry: ChosenExit | undefined, checked: boolean, all: PeerInfo[]): ExitCandidate {
  const source: Partial<PeerInfo & ChosenExit> & { id: string } = peer || stateEntry || { id_full: idFull, id: idFull.slice(0, 10) };
  return {
    idFull,
    label: peerLabel(source, true, all),
    country: source.country || null,
    online: peer ? !!peer.online : !!(stateEntry && stateEntry.online),
    active: !!(stateEntry && stateEntry.active),
    path: source.path || null,
    rttMs: source.rtt_ms != null ? source.rtt_ms : null,
    checked,
  };
}

// Candidates — chosen ∪ remembered ∪ members with `exit === true`, no repeats,
// the own node is excluded.
export function exitCandidates(state: Partial<NodeState> | null, chosen: string[], remembered: string[] | null | undefined): ExitCandidate[] {
  const st: Partial<NodeState> = state || {};
  const peers = (st.peers || []).filter((p) => !p.self);
  const byPeer = indexBy<PeerInfo>(peers, "id_full");
  const byRoute = indexBy<ChosenExit>((st.route && st.route.exits) || [], "id_full");
  const offered = peers.filter((p) => p.exit === true).map((p) => p.id_full);
  const chosenSet = new Set(chosen);
  return uniq([...chosen, ...(remembered || []), ...offered]).map((id) =>
    exitCandidate(id, byPeer.get(id), byRoute.get(id), chosenSet.has(id), st.peers || []));
}

// In a network, but no other member is known yet (right after joining): the exit list is still being filled.
export function noPeersYet(state: Partial<NodeState> | null): boolean {
  return !!(state && state.network) && !(state.peers || []).some((p) => !p.self);
}

// The own node isn't offered as an exit.
function okExitItems(state: Partial<NodeState> | null, checks: Checks): ExitItem[] {
  const all = (state && state.peers) || [];
  const others = all.filter((p) => p.self !== true);
  const ok = others.filter((p) => checks[p.id]?.verdict === "ok");
  return ok.map((p) => ({ idFull: p.id_full, label: peerLabel(p, false, all) }));
}

function view(kind: "none" | "checking" | "empty", domain: string): FindExitView;
function view(kind: "ok", domain: string, items: ExitItem[]): FindExitView;
function view(kind: FindKind, domain: string, items: ExitItem[] = []): FindExitView {
  return { kind, domain, items } as FindExitView;
}

// The exit to pick on its own: the online, not blocked candidate with the lowest latency (no latency — last);
// `null` when there is none.
export function autoExitPick(candidates: ExitCandidate[]): string | null {
  const usable = candidates.filter((c) => c.online && !c.blocked);
  const rank = (c: ExitCandidate): number => (c.rttMs == null ? Infinity : c.rttMs);
  const best = [...usable].sort((a, b) => rank(a) - rank(b))[0];
  return best ? best.idFull : null;
}

// Who gets focus on "Choose another": the first other online member, otherwise "As usual" ("").
export function pickOtherExit(candidates: ExitCandidate[]): string {
  const other = candidates.find((c) => c.online && !c.checked);
  return other ? other.idFull : "";
}

export function otherExitSelector(candidates: ExitCandidate[]): string {
  const id = pickOtherExit(candidates);
  return id ? 'input.exit-choice[data-id="' + id + '"]' : "#exit-direct";
}

// The "Check a site" result: none — no domain; checking — no verdict yet; empty — through no one;
// ok — the list of members through which the site opens.
export function findExitView(state: Partial<NodeState> | null, domain: string): FindExitView {
  if (!domain) return view("none", domain);
  const entry = ((state && state.domains) || []).find((d) => d.name === domain);
  return findVerdictView(state, domain, entry);
}

function findVerdictView(state: Partial<NodeState> | null, domain: string, entry: DomainInfo | undefined): FindExitView {
  if (!entry || entry.checking) return view("checking", domain);
  return checkedView(state, domain, entry.checks || {});
}

function checkedView(state: Partial<NodeState> | null, domain: string, checks: Checks): FindExitView {
  if (!Object.keys(checks).some((id) => checks[id])) return view("checking", domain);
  const items = okExitItems(state, checks);
  return items.length ? view("ok", domain, items) : view("empty", domain);
}
