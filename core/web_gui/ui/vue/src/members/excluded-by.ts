/* Who removed this computer from a Managed network: `network.excluded_by` is a name or id, or `{ id, name }`. */

import type { NetworkInfo } from "../core/api-types.ts";

const SHORT_ID = 8;

/** The admin's name, or the short id when the node gave no name; `null` — the node did not say. */
export function excludedByName(by: NetworkInfo["excluded_by"]): string | null {
  if (!by) return null;
  if (typeof by === "string") return by;
  return by.name?.trim() || by.id.slice(0, SHORT_ID);
}

export const excludedByText = (by: string, net: string): string => `An admin (${by}) removed this computer from "${net}". To come back, ask an admin for a new invite.`;
