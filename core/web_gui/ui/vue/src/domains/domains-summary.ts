/* The "Site checks" summary of one site: does it open in your browser now, and what to press if not.
 * Pure: the data comes from the caller. */

import { agoText, peerLabel } from "../common/words.ts";
import type { ChosenExit, PeerInfo } from "../core/api-types.ts";
import {
  DOMAINS_ALT_NONE, DOMAINS_ALT_UNCHECKED, DOMAINS_COL_VIA_SUB, DOMAINS_COL_YOU, DOMAINS_COL_YOU_SUB,
  domainsAlsoViaText, domainsOpensViaText, domainsViaHeadText,
} from "./domains-texts.ts";
import { directMark, ownCheck, verdictView } from "./domains-view.ts";
import type { OthersText } from "./domains-texts.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";

export type SummaryKind = "opens" | "blocked" | "unknown" | "checking" | "exit-offline" | "exit-not-sharing";

export interface ExitPath {
  /** The short id of the exit, empty for the direct path. */
  id: string;
  idFull: string;
  name: string;
  state: "active" | "offline" | "not-sharing" | "direct";
}

export interface SiteSummary {
  kind: SummaryKind;
  /** The mark of the result (`opens`, `blocked`, `unknown`); absent for the other kinds. */
  mark: ReturnType<typeof verdictView> | null;
  age: string;
  /** The line for the "Other exits" column. */
  others: OthersText;
  /** The member for the "Exit via …" button; only in the proxy network and only when the site does not open now. */
  button: { idFull: string; name: string } | null;
}

const DIRECT: ExitPath = { id: "", idFull: "", name: "", state: "direct" };

function pathOf(e: ChosenExit, peers: PeerInfo[], state: ExitPath["state"]): ExitPath {
  return { id: e.id, idFull: e.id_full, name: peerLabel(e, true, peers), state };
}

/** The path of the browser as the status line reads it: the active exit, else the first chosen one, else direct. */
export function exitPath(nets: SiteNet[], proxy: string, exits: ChosenExit[]): ExitPath {
  const peers = nets.find((n) => n.slot === proxy)?.peers || [];
  const active = exits.find((e) => e.active);
  if (active) return pathOf(active, peers, "active");
  if (exits.length === 0) return DIRECT;
  return pathOf(exits[0], peers, exits[0].online === false ? "offline" : "not-sharing");
}

/** The name of the member behind `idFull`, as the buttons show it; the proxy network is searched first. */
export function exitMemberName(nets: SiteNet[], proxy: string, idFull: string): string {
  const ordered = [...nets.filter((n) => n.slot === proxy), ...nets.filter((n) => n.slot !== proxy)];
  for (const net of ordered) {
    const peer = net.peers.find((p) => p.id_full === idFull);
    if (peer) return peerLabel(peer, true, net.peers);
  }
  return idFull;
}

/** The header of the first column: `Via NAME` + `your exit`, or `For you` + `no exit chosen`. */
export function viaHead(path: ExitPath): { text: string; sub: string } {
  if (path.state === "direct") return { text: DOMAINS_COL_YOU, sub: DOMAINS_COL_YOU_SUB };
  return { text: domainsViaHeadText(path.name), sub: DOMAINS_COL_VIA_SUB };
}

interface Alt {
  slot: string;
  peer: PeerInfo;
  name: string;
}

// Members of all networks that are online and open the site, except the current exit; the proxy network first.
function alternatives(d: MergedDomain, nets: SiteNet[], proxy: string, path: ExitPath): { alts: Alt[]; unchecked: boolean } {
  const alts: Alt[] = [];
  let unchecked = false;
  for (const net of nets) {
    for (const peer of net.peers) {
      if (net.slot === proxy && peer.id === path.id) continue;
      const check = d.byNet[net.slot]?.checks?.[peer.id];
      if (!check) unchecked = unchecked || peer.online;
      if (peer.online && check?.verdict === "ok") alts.push({ slot: net.slot, peer, name: peerLabel(peer, true, net.peers) });
    }
  }
  return { alts: [...alts.filter((a) => a.slot === proxy), ...alts.filter((a) => a.slot !== proxy)], unchecked };
}

function checkOf(d: MergedDomain, nets: SiteNet[], proxy: string, path: ExitPath) {
  if (path.state === "direct") return ownCheck(d, nets).check;
  return d.byNet[proxy]?.checks?.[path.id] ?? undefined;
}

function kindOf(d: MergedDomain, path: ExitPath, check: ReturnType<typeof checkOf>): SummaryKind {
  if (path.state === "offline") return "exit-offline";
  if (path.state === "not-sharing") return "exit-not-sharing";
  if (d.checking) return "checking";
  if (!check) return "unknown";
  return check.verdict === "ok" ? "opens" : "blocked";
}

function othersText(kind: SummaryKind, alts: Alt[], unchecked: boolean): OthersText {
  if (kind === "opens") return domainsAlsoViaText(alts.length);
  if (kind === "checking") return domainsAlsoViaText(0);
  if (alts.length > 0) return domainsOpensViaText(alts[0].name, alts.length - 1);
  return { lead: unchecked ? DOMAINS_ALT_UNCHECKED : DOMAINS_ALT_NONE, more: "" };
}

export function siteSummary(d: MergedDomain, nets: SiteNet[], proxy: string, exits: ChosenExit[]): SiteSummary {
  const path = exitPath(nets, proxy, exits);
  const check = checkOf(d, nets, proxy, path);
  const kind = kindOf(d, path, check);
  const { alts, unchecked } = alternatives(d, nets, proxy, path);
  const first = alts[0];
  const canPress = kind !== "opens" && kind !== "checking" && first !== undefined && first.slot === proxy;
  const shown = kind === "opens" || kind === "blocked" || kind === "unknown";
  return {
    kind,
    mark: shown ? (path.state === "direct" ? directMark(d, nets) : verdictView(check)) : null,
    age: shown ? agoText(check?.age_sec) : "",
    others: othersText(kind, alts, unchecked),
    button: canPress ? { idFull: first.peer.id_full, name: first.name } : null,
  };
}
