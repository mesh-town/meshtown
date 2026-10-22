/* Progress of joining by invite for Home and the setup — a pure function without Vue. Three phases: the node waits for members
 * (`node.invite_join.state === "waiting"`), the invite is sent and the node restarts (`JoinSent`, while there is no
 * running node with a new `boot_id`), the node refused (`refused`). Anything else is `null`. */

import { quotedNet } from "../common/words.ts";
import { ENTER_SUB, HOME_PROGRESS_ENTERING, HOME_PROGRESS_SEARCHING, SEARCH_SUB, enteringMainText, searchingMainText } from "./home-flow-texts.ts";
import type { Absence } from "../core/store.ts";
import type { InviteJoinInfo, NodeState } from "../core/api-types.ts";

export type JoinPhase = "searching" | "entering" | "refused";
export interface JoinProgress {
  phase: JoinPhase;
  main: string;
  sub: string;
}
/** Join sent: `waiting` — the node answered
 * "waiting for members", and until the first snapshot with `invite_join` (or until the restart) search is shown, not joining. */
export interface JoinSent {
  bootIdAtSend: string | null;
  waiting?: boolean;
}

export { ENTER_SUB, SEARCH_SUB };
export const PROGRESS_HEAD: Record<"searching" | "entering", string> = { searching: HOME_PROGRESS_SEARCHING, entering: HOME_PROGRESS_ENTERING };

type Snapshot = Partial<NodeState> | null;

const snapshotBootId = (state: Snapshot): string | null => (typeof state?.boot_id === "string" ? state.boot_id : null);

// The node works after joining: a new `boot_id`, not silent, not `starting`/`stopping`.
function running(state: Snapshot, sent: JoinSent, absent: Absence | null): boolean {
  const boot = snapshotBootId(state);
  const status = state?.node?.status;
  return boot !== null && boot !== sent.bootIdAtSend && !absent && status !== "stopping" && status !== "starting";
}

function labelOf(state: Snapshot, info: InviteJoinInfo | null | undefined): string {
  return quotedNet(info?.label) || quotedNet(state?.network?.label);
}

function searching(label: string): JoinProgress {
  return { phase: "searching", main: searchingMainText(label), sub: SEARCH_SUB };
}

function entering(label: string): JoinProgress {
  return { phase: "entering", main: enteringMainText(label), sub: ENTER_SUB };
}

// The "waiting for members" response has arrived but the snapshot with `invite_join` has not: the same node start, not a restart.
function awaitingSnapshot(state: Snapshot, sent: JoinSent, absent: Absence | null): boolean {
  const status = state?.node?.status;
  return !!sent.waiting && snapshotBootId(state) === sent.bootIdAtSend && !absent && status !== "stopping" && status !== "starting";
}

export function joinProgress(state: Snapshot, sent: JoinSent | null = null, absent: Absence | null = null): JoinProgress | null {
  const info = state?.node?.invite_join;
  if (info?.state === "refused") return { phase: "refused", main: info.message, sub: "" };
  if (info?.state === "waiting") return searching(labelOf(state, info));
  if (sent && awaitingSnapshot(state, sent, absent)) return searching(labelOf(state, info));
  return sent && !running(state, sent, absent) ? entering(labelOf(state, info)) : null;
}
