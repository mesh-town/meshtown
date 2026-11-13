/* The chosen exit that is overloaded: the panel lines by the table "record / local sign" — pure functions. */

import type { ChosenExit } from "../core/api-types.ts";
import { PEERS_OVERLOADED_LIMIT, PEERS_OVERLOADED_MEMORY, PEERS_OVERLOADED_OTHER, PEERS_OVERLOADED_SHARE } from "../peers/peers-texts.ts";
import { BUSY_MAY_NOT_LOAD, busyAdviceMemoryText, busyAdviceText, busyTurnedAwayText } from "./exit-busy-texts.ts";

const REASONS: Record<string, string> = {
  limit: PEERS_OVERLOADED_LIMIT,
  share: PEERS_OVERLOADED_SHARE,
  memory: PEERS_OVERLOADED_MEMORY,
};

const reasonText = (reason: string): string => REASONS[reason] || PEERS_OVERLOADED_OTHER;

/** The exit is online and overloaded (a record or the local sign). */
export function exitIsBusy(exit: ChosenExit | null | undefined): exit is ChosenExit {
  return !!exit && exit.online !== false && exit.busy === true;
}

/** The second line: the local count (if any), the reason of the record (if any), and what may go wrong. */
export function busySub(exit: ChosenExit, who: string): string {
  const away = exit.turned_away ? [busyTurnedAwayText(who, exit.turned_away.refused, exit.turned_away.attempts)] : [];
  const reason = exit.overloaded ? [reasonText(exit.overloaded)] : [];
  return [...away, ...reason, BUSY_MAY_NOT_LOAD].join(" ");
}

/** The line "what to do": the tail depends on the reason. */
export function busyHint(exit: ChosenExit, who: string): string {
  return exit.overloaded === "memory" ? busyAdviceMemoryText(who) : busyAdviceText(who);
}

export interface BusyLines {
  sub: string;
  hint: string;
}

export function busyLines(exit: ChosenExit, who: string): BusyLines {
  return { sub: busySub(exit, who), hint: busyHint(exit, who) };
}
