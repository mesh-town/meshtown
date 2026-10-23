/* The second request of the legacy-mode poll cycle: `GET /api/members` into `store.members`. Never throws; never touches `conn`. */

import { ApiError } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { getMembers } from "./members-api.ts";

// 404: an old node without the route; 409: no network or still starting.
const isNoList = (err: unknown): boolean => err instanceof ApiError && (err.status === 404 || err.status === 409);

export async function fetchMembers(store: Store, api: ApiFn): Promise<void> {
  if (!store.state || !store.state.network) {
    store.members = null;
    return;
  }
  try {
    store.members = await getMembers(api);
  } catch (err) {
    if (isNoList(err)) store.members = null;
  }
}
