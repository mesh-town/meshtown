/* "This computer is no longer in the network" for the views (Members, Home, Settings): the flag, the card variant and the
 * network name. Reads the store only; does not change it. */

import { computed } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../core/store.ts";
import type { ExcludedVariant } from "./members-ids.ts";
import { excludedVariant, isMeExcluded, meId } from "./members-view.ts";

interface ExcludedView {
  excluded: ComputedRef<boolean>;
  variant: ComputedRef<ExcludedVariant>;
  net: ComputedRef<string>;
}

export function useExcluded(store: Store): ExcludedView {
  return {
    excluded: computed(() => isMeExcluded(store.state, store.members)),
    variant: computed(() => excludedVariant(store.members, meId(store.state))),
    net: computed(() => (store.state && store.state.network && store.state.network.label) || ""),
  };
}
