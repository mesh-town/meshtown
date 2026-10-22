/* The "waiting for network members" line under the join card: joining by the single-use invite `mesh-invite2-`
 * happens in the node, not on the page, and is visible in the snapshot in the `node.invite_join` field. A pure function without Vue. */

import type { InviteJoinInfo } from "../core/api-types.ts";
import { quotedNet } from "../common/words.ts";
import { inviteWaitingText } from "./home-flow-texts.ts";

// Text for `#join-wait`: waiting — a hint (with the network name, if the invite has it),
// refusal — the node message verbatim, no join — an empty string (the element is hidden).
export function inviteWaitText(info: InviteJoinInfo | null | undefined): string {
  if (!info) return "";
  if (info.state === "refused") return info.message;
  if (info.state !== "waiting") return "";
  return inviteWaitingText(quotedNet(info.label));
}
