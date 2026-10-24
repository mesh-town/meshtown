/* Acknowledging a notice: the node drops it, the page drops it from the store at once (the next poll confirms). */

import { api, ApiError } from "../core/api.ts";
import type { Store } from "../core/store.ts";

export async function ackNotice(store: Store, id: number): Promise<boolean> {
  try {
    await api("POST", "/api/notices/ack", { id });
  } catch (err) {
    // 404: the node no longer has the notice, so it is gone either way.
    if (!(err instanceof ApiError && err.code === "not_found")) return false;
  }
  store.notices = store.notices.filter((n) => n.id !== id);
  return true;
}
