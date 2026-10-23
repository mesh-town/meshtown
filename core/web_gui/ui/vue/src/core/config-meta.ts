/* Config documents in the store: the lists around a config reply and a slot's slice. */

import type { ConfigMeta, ConfigResponse } from "./api-types.ts";
import type { Store } from "./store.ts";

const CONFIG_META_LISTS = ["read_only", "restart_fields", "restart_pending", "cli_overrides"] as const;

export function configMeta(data: ConfigResponse): ConfigMeta {
  const meta: Partial<ConfigMeta> = {};
  for (const key of CONFIG_META_LISTS) meta[key] = data[key] || [];
  return meta as ConfigMeta;
}

/** Puts a config reply into the slot's slice; `bootId` is the boot the document was read at. False — no such slice. */
export function storeSlotConfig(store: Store, id: string, data: ConfigResponse, bootId: string | null): boolean {
  const slice = store.slots[id];
  if (!slice) return false;
  slice.config = data.config;
  slice.configRev = data.rev;
  slice.configMeta = configMeta(data);
  slice.bootId = bootId;
  return true;
}
