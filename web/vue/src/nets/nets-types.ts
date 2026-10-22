/* Types of the network selection: node contract types (`core/api-types-state.ts`) and narrow slices for pure functions. */

import type { NodeRuntime, SocksInfo, WebState } from "../core/api-types.ts";

export type { NetBy, NetEntry, WebState } from "../core/api-types.ts";

/** The slice of `NodeState` needed for the network selection; `store.state` passes without a cast, so does an older node. */
export interface NetsInput {
  node?: Partial<NodeRuntime>;
  socks?: Partial<SocksInfo>;
  web?: Partial<WebState>;
}

/** The slice of `ConfigDocument`: the saved access and network lists (`[]` or no key — automatic). */
export interface SavedNets {
  socks?: { access?: string; ifaces?: string[] | null };
  web?: { access?: string; ifaces?: string[] | null };
}
