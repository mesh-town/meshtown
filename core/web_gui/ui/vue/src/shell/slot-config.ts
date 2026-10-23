/* Reading the config document of an added network slot into its slice of the store. */

import type { ConfigMeta, ConfigResponse } from "../core/api-types.ts";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { slotPath } from "../core/slots.ts";

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
