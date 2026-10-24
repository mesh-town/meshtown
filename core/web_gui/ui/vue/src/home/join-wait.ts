/* The "waiting for network members" line under the join card: joining by the single-use invite `mesh-invite2-`
 * happens in the node, not on the page, and is visible in the snapshot in the `node.invite_join` field. A pure function without Vue. */

import type { InviteJoinInfo } from "../core/api-types.ts";
import { quotedNet } from "../common/words.ts";
import { inviteWaitingText } from "./home-flow-texts.ts";
import { REFUSED_LEAD } from "./invite-join-texts.ts";

// Text for `#join-wait`: waiting — a hint (with the network name, if the invite has it),
// refusal — the node message verbatim, no join — an empty string (the element is hidden).
export function inviteWaitText(info: InviteJoinInfo | null | undefined): string {
  if (!info) return "";
  if (info.state === "refused") return info.message;
  if (info.state !== "waiting") return "";
  return inviteWaitingText(quotedNet(info.label));
}

// The bold lead before a refusal message of the node; empty while there is no refusal.
export function refusedLead(info: InviteJoinInfo | null | undefined): string {
  return info?.state === "refused" ? REFUSED_LEAD(quotedNet(info.label)) : "";
}

// The refusal message of a join, `null` while there is none.
export function refusedText(info: InviteJoinInfo | null | undefined): string | null {
  return info?.state === "refused" ? info.message : null;
}
