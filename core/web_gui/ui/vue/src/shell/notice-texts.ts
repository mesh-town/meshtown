/* Texts of the strip of notices: a network an admin deleted. */

import { forDevice } from "../common/device.ts";
import type { Notice } from "../core/api-types.ts";

export const NOTICE_OK = "OK";

/** `"family" was deleted by an admin (anna). This computer has left it.`; no admin name — no brackets. */
export function noticeText(n: Notice): string {
  const by = n.by_name && n.by_name.trim() ? ` (${n.by_name.trim()})` : "";
  return `"${n.label ?? "network"}" was deleted by an admin${by}. ` + forDevice("This computer has left it.");
}

/** The oldest first. */
export const sortedNotices = (list: Notice[]): Notice[] => [...list].sort((a, b) => a.at_unix - b.at_unix || a.id - b.id);
