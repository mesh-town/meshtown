/* Acknowledging a notice: the node drops it, the page drops it from the store at once (the next poll confirms). */

import { api } from "../core/api.ts";
import type { Store } from "../core/store.ts";

export async function ackNotice(store: Store, id: number): Promise<boolean> {
  try {
    await api("POST", "/api/notices/ack", { id });
  } catch {
    return false;
  }
  store.notices = store.notices.filter((n) => n.id !== id);
  return true;
}
