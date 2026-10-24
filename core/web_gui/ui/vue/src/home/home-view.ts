/* The "Home" layout by snapshot — pure functions without Vue: the mode (setup or normal view),
 * the setup step, the "how sites open right now" panel, the place of the join card, the card order. */

import {
  ABSENCE_HINT, ABSENCE_TEXT, CLI_FIXED_NOTE, EXIT_DOWN_TAIL, ROUTE_DIRECT_TEXT, ROUTE_LINE_APP_DIRECT, ROUTE_LINE_DIRECT,
  peerLabel, routeLineAppViaText, routeLineViaText, routeViaText,
} from "../common/words.ts";
import type { Absence } from "../core/store.ts";
import type { ChosenExit, ConfigDocument, ConfigMeta, NetBrief, NodeState, PeerInfo, SlotSlice } from "../core/api-types.ts";
import { netName } from "../core/slots.ts";
import { getPath } from "../settings/settings-fields.ts";
import { metaHas } from "../settings/settings-view.ts";
import { inAppPage } from "./browser-setup.ts";
import { exitCandidates } from "./exit-list.ts";
import type { SetupMark } from "./home-actions.ts";
import {
  HOME_DIRECT_NONE, HOME_DIRECT_PICK, HOME_EXITS_OFFLINE, HOME_EXIT_CHOSEN_FALLBACK, HOME_LINK_DIRECT, HOME_LINK_ONLINE, HOME_LINK_RELAY,
  HOME_WAIT_STARTING, HOME_WAIT_STOPPING, offlinePeerText, rttMsText,
} from "./home-flow-texts.ts";
import type { JoinProgress } from "./join-progress.ts";
import type { RouteHead } from "./route-line.ts";
import {
  NETS_DIRECT, NETS_JUST_YOU, NETS_NO_CONNECTION, netsAdmittingTail, netsChipShort, netsChipText, netsExitOfflineText, netsExitRemovedText,
  netsNoLinkText, netsOnlineText, netsViaText,
} from "./nets-texts.ts";

type Snapshot = Partial<NodeState> | null;
export type HomeMode = "wizard" | "normal";
export type PanelLevel = "green" | "red" | "grey";
export type PanelAction = "pick-other" | "direct";
export interface StatePanelView {
  level: PanelLevel;
  main: string;
  /** The short route phrase for the header. */
  line?: string;
  sub: string;
  actions: PanelAction[];
  busy?: boolean;
}
export type CardKey = "nets" | "invite" | "exit" | "row";
export type OrderVariant = "A" | "B" | "V";

// The name from settings, otherwise from the handshake (the `peers[]` row with `self`).
export function nodeNameFieldValue(config: ConfigDocument | null, state: Snapshot): string {
  const fromConfig = getPath(config, "node.name") as string | null | undefined;
  return fromConfig !== null && fromConfig !== undefined ? fromConfig : handshakeName(state || {});
}

function handshakeName(st: Partial<NodeState>): string {
  const self = (st.peers || []).find((p) => p.self);
  return (self && self.name) || "";
}

// A settings field by `configMeta`: read-only and the "set at node startup" note.
export function configFieldAccess(meta: ConfigMeta | null | undefined, path: string): { readOnly: boolean; cliNote: string } {
  return {
    readOnly: metaHas(meta, "read_only", path),
    cliNote: metaHas(meta, "cli_overrides", path) ? CLI_FIXED_NOTE : "",
  };
}

// Card order in a network: "A" — the exit choice first, "B" — the invite first, "V" — B while
// no one lets sites through them, otherwise A (the answer to an open question may change this line).
export const HOME_ORDER: OrderVariant = "V";

const MARK_STEPS = new Map<string | null, number>([["browser", 2], ["exit", 3]]);

function hasNet(state: Snapshot): boolean {
  return !!(state && state.network);
}

// The setup step 1…3; 0 — the setup is not needed.
export function wizardStep(state: Snapshot, setupMark: SetupMark | string | null): number {
  if (!hasNet(state)) return 1;
  return MARK_STEPS.get(setupMark) || 0;
}

export function homeMode(state: Snapshot, setupMark: SetupMark | string | null): HomeMode {
  return wizardStep(state, setupMark) ? "wizard" : "normal";
}

export type HomeTab = "main" | "wizard";

// The open tab of "Home": an explicit choice, otherwise by mode (the setup — the setup tab).
export function activeTabOf(explicit: HomeTab | null, mode: HomeMode): HomeTab {
  return explicit || (mode === "wizard" ? "wizard" : "main");
}

// The step to show in the setup: a completed setup opened by hand is shown from the second step.
export function shownStep(step: number): number {
  return step || 2;
}

// The join form always lives on Home; with a network, Settings offers the "add another network" card instead.
export function joinPlace(_state: Snapshot): "settings" | "home" {
  return "home";
}

function panel(level: PanelLevel, main: string, sub = "", actions: PanelAction[] = []): StatePanelView {
  return { level, main, line: main, sub, actions };
}

const WAIT_TEXT = new Map<string | undefined, string>([
  ["starting", HOME_WAIT_STARTING],
  ["stopping", HOME_WAIT_STOPPING],
]);

// The join progress by invite: searching and joining — a grey panel with a spinner, a node refusal — a red one.
function progressPanel(p: JoinProgress): StatePanelView {
  const refused = p.phase === "refused";
  return { level: refused ? "red" : "grey", main: p.main, line: p.main, sub: p.sub, actions: [], busy: !refused };
}

function absencePanel(absent: Absence): StatePanelView {
  return panel("grey", ABSENCE_TEXT[absent], ABSENCE_HINT[absent]);
}

/** With several networks: names the network of an exit and whether that network has lost its link. */
export interface PanelCtx {
  multi: boolean;
  netOf: (peerId: string) => string | null;
  linkDown?: (peerId: string) => boolean;
  /** The header verdict; a red one means sites will not open, whatever the leading network says. */
  head?: RouteHead | null;
}

function directPanel(state: Partial<NodeState>, ctx?: PanelCtx): StatePanelView {
  if (ctx?.multi) return { ...panel("green", NETS_DIRECT, HOME_DIRECT_PICK) };
  const some = exitCandidates(state, [], []).length > 0;
  const line = inAppPage() ? ROUTE_LINE_APP_DIRECT : ROUTE_LINE_DIRECT;
  return { ...panel("green", inAppPage() ? line : ROUTE_DIRECT_TEXT, some ? HOME_DIRECT_PICK : HOME_DIRECT_NONE), line };
}

function linkTail(peer: PeerInfo | undefined): string {
  if (!peer) return "";
  const rtt = peer.rtt_ms != null ? [rttMsText(peer.rtt_ms)] : [];
  const path = peer.path === "direct" ? [HOME_LINK_DIRECT] : peer.path === "relay" ? [HOME_LINK_RELAY] : [];
  return [HOME_LINK_ONLINE, ...path, ...rtt].join(" · ");
}

function exitPanel(state: Partial<NodeState>, ctx?: PanelCtx): StatePanelView {
  const active = (state.route?.exits || []).find((e) => e.active);
  const peers = state.peers || [];
  const who = active ? peerLabel(active, false, peers) : HOME_EXIT_CHOSEN_FALLBACK;
  const peer = peers.find((p) => active && p.id_full === active.id_full);
  const net = active && ctx?.multi ? ctx.netOf(active.id_full) : null;
  if (net !== null) return { ...panel("green", netsViaText(who, net), linkTail(peer)) };
  const app = inAppPage();
  const line = app ? routeLineAppViaText(who) : routeLineViaText(who);
  return { ...panel("green", app ? line : routeViaText(who), linkTail(peer)), line };
}

function multiUnavailable(exit: ChosenExit, who: string, ctx: PanelCtx): StatePanelView | null {
  const net = ctx.netOf(exit.id_full);
  if (net === null) return null;
  const text = exit.removed ? netsExitRemovedText(who, net) : ctx.linkDown?.(exit.id_full) ? netsNoLinkText(who, net) : netsExitOfflineText(who, net);
  return panel("red", text, "", ["pick-other"]);
}

function unavailablePanel(state: Partial<NodeState>, ctx?: PanelCtx): StatePanelView {
  const exits: ChosenExit[] = state.route?.exits || [];
  const multi = ctx?.multi && exits.length === 1 ? multiUnavailable(exits[0], peerLabel(exits[0], false, state.peers || []), ctx) : null;
  if (multi) return multi;
  const who = exits.length === 1 ? offlinePeerText(peerLabel(exits[0], false, state.peers || [])) : HOME_EXITS_OFFLINE;
  return panel("red", who + " — " + EXIT_DOWN_TAIL, "", ["pick-other", "direct"]);
}

const ROUTE_PANELS = new Map<string | undefined, (state: Partial<NodeState>, ctx?: PanelCtx) => StatePanelView>([["exit", exitPanel], ["unavailable", unavailablePanel]]);

function routePanel(state: Partial<NodeState>, ctx?: PanelCtx): StatePanelView {
  const route = state.route;
  if (ctx?.multi && ctx.head?.level === "red") return panel("red", ctx.head.text, "", ["pick-other"]);
  return (ROUTE_PANELS.get(route && route.state) || directPanel)(state, ctx);
}

// `{ level: "green"|"red"|"grey", main, sub, actions }`; an empty snapshot gives no exception.
// `absent` — the page gets no snapshots (`store.absence`): it comes before the snapshot.
export function statePanel(state: Snapshot, absent: Absence | null = null, progress: JoinProgress | null = null, ctx?: PanelCtx): StatePanelView {
  if (progress) return progressPanel(progress);
  if (absent) return absencePanel(absent);
  if (!state) return absencePanel("silent");
  const wait = WAIT_TEXT.get(state.node && state.node.status);
  return wait ? panel("grey", wait) : routePanel(state, ctx);
}

function hasOtherExit(state: Snapshot): boolean {
  const peers = (state && state.peers) || [];
  const exits = (state && state.route && state.route.exits) || [];
  return exits.length > 0 || peers.some((p) => !p.self && p.exit === true);
}

// "invite" first — the invite as a separate card above the choice; otherwise it is in the row of small ones ("row").
export const ORDERS = new Map<OrderVariant, (state: Snapshot) => CardKey[]>([
  ["A", () => ["exit", "row"]],
  ["B", () => ["invite", "exit", "row"]],
  ["V", (state) => (hasOtherExit(state) ? ["exit", "row"] : ["invite", "exit", "row"])],
]);

export function cardOrder(state: Snapshot, variant: OrderVariant, multi = false): CardKey[] {
  if (multi) return ["nets", "exit", "row"];
  return (ORDERS.get(variant) || ORDERS.get("V")!)(state);
}

/** A network line on Home. */
export interface NetRowView {
  slot: string;
  title: string;
  summary: string;
  dot: "green" | "grey";
  admitting: number;
}

function netSummary(brief: NetBrief): string {
  if (brief.peers_total === 0) return NETS_JUST_YOU;
  return brief.peers_online > 0 ? netsOnlineText(brief.peers_online, brief.peers_total) : NETS_NO_CONNECTION;
}

// No slice yet (nothing to talk to) reads as "no connection".
export function netRowView(brief: NetBrief, slot: SlotSlice | undefined, title: string = netName([brief], brief.id)): NetRowView {
  const known = slot?.state != null;
  const base = known ? netSummary(brief) : NETS_NO_CONNECTION;
  const admitting = brief.members_admitting;
  return {
    slot: brief.id,
    title,
    summary: base + (known && admitting > 0 ? netsAdmittingTail(admitting) : ""),
    dot: known && brief.peers_online > 0 ? "green" : "grey",
    admitting,
  };
}

export interface StatusChip {
  slot: string;
  text: string;
  short: string;
}

// One chip per shown network that has requests to confirm; `shown` limits the nets to the visible slots.
export function statusChips(nets: NetBrief[], shown?: string[]): StatusChip[] {
  return nets
    .filter((n) => n.id_hex != null && n.members_admitting > 0 && (shown === undefined || shown.includes(n.id)))
    .map((n) => ({ slot: n.id, text: netsChipText(n.members_admitting, netName(nets, n.id)), short: netsChipShort(n.members_admitting, netName(nets, n.id)) }));
}
