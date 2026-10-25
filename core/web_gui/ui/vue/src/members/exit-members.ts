/* What the member list means for the exit you chose: a warning when it was banned, left or is blocked here,
 * the exit list without banned members, and the header line / state panel that carry the warning. Pure functions. */

import { forDevice } from "../common/device.ts";
import type { ExitCandidate } from "../home/exit-list.ts";
import type { StatePanelView } from "../home/home-view.ts";
import type { StatusLine } from "../shell/status-line.ts";
import type { MemberRow, MembersReply } from "./members-types.ts";
import { displayNames } from "./members-ids.ts";

type ExitWarningKind = "removed" | "left" | "blocked";

export interface ExitWarning {
  id: string;
  kind: ExitWarningKind;
  text: string;
  hint: string;
}

const TAIL = " — sites won't open until you choose another exit";
const WARNING_TAIL: Record<ExitWarningKind, string> = {
  removed: "was banned from the network",
  left: "left the network",
  blocked: "is blocked on this computer",
};

// Excluded wins over blocked: a banned member is reported as banned or left.
function kindOf(row: MemberRow | undefined): ExitWarningKind | null {
  if (!row) return null;
  if (row.status === "excluded") return row.excluded && row.excluded.how === "left" ? "left" : "removed";
  return isBlocked(row) ? "blocked" : null;
}

// Blocked here: by an explicit block, or by my own ban or removal proposal.
function isBlocked(row: MemberRow): boolean {
  return !!row.blocked_by_me || row.denied_by_me === "voted_by_me" || row.denied_by_me === "removed_by_me";
}

function warningOf(id: string, row: MemberRow | undefined, names: Record<string, string>): ExitWarning | null {
  const kind = kindOf(row);
  if (!kind) return null;
  const name = names[id] || id.slice(0, 8);
  const what = name + " " + forDevice(WARNING_TAIL[kind]);
  return { id, kind, text: "Your exit " + what + TAIL, hint: what + ". Choose another exit below." };
}

export function exitWarning(chosen: string[], members: MembersReply | null | undefined, meExcluded: boolean): ExitWarning | null {
  if (meExcluded || !members) return null;
  const names = displayNames(members);
  const byId = new Map(members.members.map((m) => [m.id_full, m]));
  const found = chosen.map((id) => warningOf(id, byId.get(id), names));
  return found.find((w) => w !== null) || null;
}

// Banned members leave the list; blocked ones stay, marked.
export function memberExits(cands: ExitCandidate[], members: MembersReply | null | undefined): ExitCandidate[] {
  if (!members) return cands;
  const byId = new Map(members.members.map((m) => [m.id_full, m]));
  return cands.filter((c) => byId.get(c.idFull)?.status !== "excluded").map((c) => markBlocked(c, byId.get(c.idFull)));
}

function markBlocked(cand: ExitCandidate, row: MemberRow | undefined): ExitCandidate {
  return row && isBlocked(row) ? { ...cand, blocked: true } : cand;
}

// A green or red line is replaced by the warning; a grey one (starting, not responding) says more.
export function withExitWarning(line: StatusLine, w: ExitWarning | null): StatusLine {
  if (!w || line.level === "grey") return line;
  return { level: "red", text: w.text, alert: true };
}

export function warnPanel(panel: StatePanelView, w: ExitWarning | null): StatePanelView {
  if (!w || panel.level === "grey") return panel;
  const line = panel.line === undefined ? {} : { line: panel.line };
  return { level: "red", main: w.text, ...line, sub: "", actions: ["pick-other", "direct"] };
}
