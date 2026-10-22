/* The header status line and the identity strip — pure functions without Vue (a port of
 * `06-render.js`). The phrase "how sites open right now" comes from Home (`statePanel`). */

import type { ConfigFileInfo, NodeState, PeerInfo } from "../core/api-types.ts";
import type { Conn } from "./conn.ts";
import { fieldErrorLine } from "../settings/settings-fields.ts";
import { ABSENCE_TEXT, ALONE_INVITE_SHORT, ALONE_TEXT, NET_LOADING_TEXT, NO_CATALOG_SHORT, NOT_RESPONDING_TEXT, offlineOthersText, quotedNet } from "../common/words.ts";
import { IDENTITY_DECLINED_NOTE, IDENTITY_PRESERVED_NOTE, IDENTITY_PROBLEM_NOTE, restartConsequenceText } from "./shell-texts.ts";
import { excludedStatusText } from "../members/members-texts.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { statePanel } from "../home/home-view.ts";
import { netPhase } from "../home/home-words.ts";
import { joinProgress } from "../home/join-progress.ts";
import type { NetPhase } from "../home/home-words.ts";
import { STATUS_HIM, STATUS_NETWORK_SEP, STATUS_NOT_JOINED, STATUS_STARTING, STATUS_STOPPING, STATUS_THIS_COMPUTER, statusOnlineOthersText } from "./status-texts.ts";

export type StatusLevel = "green" | "red" | "grey";
export interface StatusLine {
  level: StatusLevel;
  text: string;
  alert?: boolean;
}
export interface IdentityStripView {
  cls: string;
  text: string;
}

const grey = (text: string): StatusLine => ({ level: "grey", text });

const WAIT_TEXT = new Map<string, string>([
  ["starting", STATUS_STARTING],
  ["stopping", STATUS_STOPPING],
]);

// The header speaks about the others (it does not count itself) and uses the word "more"; Home counts everyone, you included.
export function othersOnlineText(state: Partial<NodeState> | null | undefined): string {
  const rest = ((state && state.peers) || []).filter((p) => !p.self);
  const tail = OTHERS_TAIL.get(netPhase(state)) as (others: PeerInfo[]) => string;
  return tail(rest);
}

function onlineOthersText(n: number): string {
  return statusOnlineOthersText(n);
}

const OTHERS_TAIL = new Map<NetPhase, (others: PeerInfo[]) => string>([
  ["online", (rest) => onlineOthersText(rest.filter((p) => p.online).length)],
  ["loading", () => NET_LOADING_TEXT],
  ["no_catalog", (rest) => (rest.length === 0 ? NO_CATALOG_SHORT : offlineOthersText(rest.length))],
  ["offline", (rest) => offlineOthersText(rest.length)],
  ["alone", () => (inAppPage() ? ALONE_TEXT : ALONE_INVITE_SHORT)],
]);

function absenceLine(state: NodeState | null, conn: Conn): StatusLine | null {
  if (conn.awaitingToken) return grey(ABSENCE_TEXT.token);
  if (!state || (conn.strip && conn.strip.kind === "disconnected")) return grey(ABSENCE_TEXT.silent);
  return conn.stopped ? grey(ABSENCE_TEXT.stopped) : null;
}

function waitingLine(state: NodeState): StatusLine | null {
  const text = WAIT_TEXT.get(state.node && state.node.status);
  return text ? grey(text) : null;
}

// This computer was removed from the network (or left it) but the node still reports the network.
function excludedLine(state: NodeState): StatusLine | null {
  if (!state.network || !(state.members && state.members.me_excluded)) return null;
  return { level: "red", text: excludedStatusText(state.network.label) };
}

// The node is looking for the network by invite (`invite_join`) — the header says the same as Home, not "not joined yet".
function noNetworkLine(state: NodeState): StatusLine | null {
  if (state.network) return null;
  const progress = joinProgress(state);
  return grey(progress && progress.phase === "searching" ? progress.main : STATUS_NOT_JOINED);
}

// A browser header names the network without the word "network" and quotes; no label — no segment.
function plainNet(label: unknown): string {
  return typeof label === "string" && label.trim() ? " · " + label : "";
}

function joinedNetworkLine(state: NodeState & { network: NonNullable<NodeState["network"]> }): StatusLine {
  const panel = statePanel(state);
  const tail = inAppPage() ? quotedNet(state.network.label).replace(/^ /, STATUS_NETWORK_SEP) : plainNet(state.network.label);
  return { level: panel.level, text: panel.line + tail + " · " + othersOnlineText(state) };
}

// `{ level: "green"|"red"|"grey", text }` — in the order of the branches.
export function statusLine(state: NodeState | null, conn: Conn): StatusLine {
  return absenceLine(state, conn) || (state && (waitingLine(state) || noNetworkLine(state) || excludedLine(state) || joinedNetworkLine(state as Parameters<typeof joinedNetworkLine>[0]))) || grey(NOT_RESPONDING_TEXT);
}

function capitalizeFirst(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

// The reason text comes from the node (`file.message`/`field_errors`) and is not made up.
function reasonText(fileBlock: ConfigFileInfo | null | undefined): string {
  if (!fileBlock) return "";
  const parts = fileBlock.message ? [fileBlock.message] : [];
  for (const fe of fileBlock.field_errors || []) parts.push(fieldErrorLine(fe));
  return parts.join("; ");
}

function withReason(reason: string, fallback: string): string {
  if (reason) return capitalizeFirst(reason) + ". " + restartConsequenceText(STATUS_THIS_COMPUTER);
  return fallback + restartConsequenceText(fallback ? STATUS_HIM : STATUS_THIS_COMPUTER);
}

// The node reports only `overwrite_refused`: the advice "delete the file" is dangerous — the file may hold
// the only copy of the network key.
function preservedText(reason: string): string {
  return IDENTITY_PRESERVED_NOTE + withReason(reason, "");
}

const IDENTITY_VIEWS = new Map<string | null, (reason: string) => IdentityStripView>([
  ["declined", (reason) => ({ cls: "identity-strip-declined", text: withReason(reason, IDENTITY_DECLINED_NOTE) })],
  ["file_preserved", (reason) => ({ cls: "identity-strip-preserved", text: preservedText(reason) })],
]);

function problemView(reason: string): IdentityStripView {
  return { cls: "identity-strip-problem", text: withReason(reason, IDENTITY_PROBLEM_NOTE) };
}

// The "identity is temporary" strip → `{ cls, text }`; otherwise `null`.
export function identityStrip(state: NodeState | null, configFile: ConfigFileInfo | null | undefined): IdentityStripView | null {
  const node = state && state.node;
  if (!node || node.identity !== "temporary") return null;
  const view = IDENTITY_VIEWS.get(node.identity_reason) || problemView;
  return view(reasonText(configFile));
}
