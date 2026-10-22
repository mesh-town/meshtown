/* Page store: the node snapshot, the config, the active section and the "not saved" mark on "Settings". */

import { reactive } from "vue";
import type { InjectionKey } from "vue";
import type { MembersReply } from "../members/members-types.ts";
import type { ConfigFileInfo, ConfigDocument, ConfigMeta, NodeState } from "./api-types.ts";

// Why the page gets no snapshots: the link is stale, the node is silent, the node is stopped.
export type Absence = "token" | "silent" | "stopped";

export interface StoreState {
  state: NodeState | null;
  config: ConfigDocument | null;
  configRev: number | null;
  configMeta: ConfigMeta | null;
  configFile: ConfigFileInfo | null;
  activeSection: string;
  settingsDirty: boolean;
  absence: Absence | null;
  members: MembersReply | null; // `GET /api/members`; null — no list (old node, no network, not asked yet)
}

export type Store = StoreState;

export const STORE_KEY: InjectionKey<Store> = Symbol("meshtown.store");

export function createStore(): Store {
  return reactive<StoreState>({
    state: null,
    config: null,
    configRev: null,
    configMeta: null,
    configFile: null,
    activeSection: "home",
    settingsDirty: false, // there are unsaved edits in "Settings"
    absence: null,
    members: null,
  });
}
