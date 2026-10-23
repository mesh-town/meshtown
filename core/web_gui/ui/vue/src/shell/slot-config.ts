/* Reading the config document of an added network slot into its slice of the store. */

import type { ConfigResponse } from "../core/api-types.ts";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { storeSlotConfig } from "../core/config-meta.ts";
import { slotPath } from "../core/slots.ts";

/** Reads the slot's config. Never throws and never touches the connection: any failure is `false` and the slice stays as it was. */
export async function readSlotConfig(store: Store, api: ApiFn, id: string, bootId: string | null): Promise<boolean> {
  if (!store.slots[id]) return false;
  try {
    const data = await api<ConfigResponse | null>("GET", slotPath(id, "config"));
    return data ? storeSlotConfig(store, id, data, bootId) : false;
  } catch {
    return false;
  }
}
