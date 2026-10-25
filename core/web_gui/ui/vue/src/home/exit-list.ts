/* Exit choice on "Home" — pure functions without Vue and DOM (a port of `14-home-exit-choice.js`,
 * `14-home-exit-rows.js`, `14-home-exit.js`): the candidates of the "Exit via…" list,
 * row labels, the "Check a site" result. */

import type { ChosenExit, CheckInfo, DomainInfo, NodeState, PathKind, PeerInfo } from "../core/api-types.ts";
import { forDevice } from "../common/device.ts";
import { peerLabel } from "../common/words.ts";
import { BADGE_BLOCKED } from "../members/members-texts.ts";
import { HOME_EXIT_NOT_OFFERED, HOME_LINK_DIRECT, HOME_LINK_OFFLINE, HOME_LINK_ONLINE, HOME_LINK_RELAY, rttMsText } from "./home-flow-texts.ts";

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
  /** Chosen, the member is known, and it does not offer an exit any more. */
  notExit?: boolean;
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
export function exitMetaText(item: Pick<ExitCandidate, "online" | "path" | "rttMs" | "blocked" | "notExit">): string {
  if (item.blocked) return forDevice(BADGE_BLOCKED);
  if (!item.online) return HOME_LINK_OFFLINE;
  return notOffered(item) ? HOME_EXIT_NOT_OFFERED : onlineMeta(item);
}

// The row shows "not an exit" (and its hint) only when nothing stronger — blocked, offline — is said about it.
export function notOffered(item: Pick<ExitCandidate, "online" | "blocked" | "notExit">): boolean {
  return !!item.notExit && item.online && !item.blocked;
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
    notExit: checked && !!peer && peer.exit !== true,
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
  return uniq([...offered, ...(remembered || []), ...chosen]).map((id) =>
    exitCandidate(id, byPeer.get(id), byRoute.get(id), chosenSet.has(id), st.peers || []));
}

// A remembered id stays only while the network still offers it: a peer (not the own node) with `exit === true`.
// A state without a peer list leaves the ids as they are.
export function offeredRemembered(state: Partial<NodeState> | null, remembered: string[]): string[] {
  if (!state || !Array.isArray(state.peers)) return remembered;
  const offered = new Set(state.peers.filter((p) => !p.self && p.exit === true).map((p) => p.id_full));
  return remembered.filter((id) => offered.has(id));
}

// One network: ids the page remembered itself stay as they are; a legacy bare id stays only while the network still offers it.
export function shownRemembered(state: Partial<NodeState> | null, trusted: string[], legacy: string[]): string[] {
  return [...trusted, ...offeredRemembered(state, legacy)];
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

// The radio value of an exit: the bare id with one network, "<slot>|<id>" with two or more.
export function exitValue(slot: string, idFull: string, multi: boolean): string {
  return multi ? `${slot}|${idFull}` : idFull;
}

export function parseExitValue(value: string): { slot: string; id: string } {
  const at = value.indexOf("|");
  if (at < 0) return { slot: "main", id: value };
  return { slot: value.slice(0, at), id: value.slice(at + 1) };
}

// The radio of an exit (`id` empty — "Directly"); with a slot the row of that network.
export function otherExitSelector(id: string, slot?: string): string {
  if (!id) return "#exit-direct";
  const where = slot === undefined ? "" : '[data-slot="' + slot + '"]';
  return 'input.exit-choice[data-id="' + id + '"]' + where;
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
