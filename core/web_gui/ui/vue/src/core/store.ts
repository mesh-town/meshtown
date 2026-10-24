/* Page store: the node snapshot, the config, the active section and the "not saved" mark on "Settings". */

import { reactive } from "vue";
import type { InjectionKey } from "vue";
import type { MembersReply } from "../members/members-types.ts";
import type { ConfigFileInfo, ConfigDocument, ConfigMeta, NetBrief, NodeState, Notice, SlotSlice } from "./api-types.ts";

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
  members: MembersReply | null; // the leading slot's member list; null — no list (old node, no network, not asked yet)
  slots: Record<string, SlotSlice>; // every slot of the last overview, hidden ones too
  nets: NetBrief[];
  restartSlots: string[]; // slots with changes that need a restart
  notices: Notice[]; // notes the node keeps until they are acknowledged
  lead: string; // the leading slot: `state` and `members` are its
  peersTab: string | null;
  source: "overview" | "legacy" | null; // which request the last accepted snapshot came from
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
    slots: {},
    nets: [],
    restartSlots: [],
    notices: [],
    lead: "main",
    peersTab: null,
    source: null,
  });
}
