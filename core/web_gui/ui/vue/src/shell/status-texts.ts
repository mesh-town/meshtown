// Texts of the header status line (except the four identity texts: they stay in status-line.ts verbatim).

import { NET_NAME } from "../common/words.ts";
import { plural } from "../common/locale.ts";

export const STATUS_STARTING = NET_NAME + " is starting…";
export const STATUS_STOPPING = NET_NAME + " is stopping…";
export const STATUS_NOT_JOINED = "You haven't joined a " + NET_NAME + " network yet";
export const STATUS_NETWORK_SEP = " · network ";
export const STATUS_THIS_COMPUTER = "this computer";
export const STATUS_HIM = "this computer";

export function statusStartFailedText(reason: string): string {
  return NET_NAME + " couldn't start: " + reason + ". Details are on the \"Log\" tab.";
}

export function statusOnlineOthersText(n: number): string {
  return n + " " + plural(n, "member", "members") + " online";
}
