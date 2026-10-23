/* Wrapper around the `domain-actions.ts` core for the section: reactive state and watching
 * `rowsBusy`. `deps = { api }` can be replaced in a test. */

import { reactive, watch } from "vue";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { api as coreApi } from "../core/api.ts";
import { targetSlots } from "../core/save-config.ts";
import { isMulti, slotState } from "../core/slots.ts";
import { createDomainActions, emptyUi } from "./domain-actions.ts";
import type { DomainActions, DomainTargets, DomainUi } from "./domain-actions.ts";
import { mergedDomains, rowsBusy, siteNets } from "./domains-view.ts";

export type UseDomainActions = { ui: DomainUi } & DomainActions;

// One network: only the leading slot; several: every visible slot with a key.
function targetsOf(store: Store): DomainTargets {
  const all = (): string[] => (isMulti(store) ? targetSlots(store) : [store.lead]);
  const names = (slot: string): string[] => {
    const state = isMulti(store) ? slotState(store, slot) : store.state;
    return (state?.domains || []).map((d) => d.name);
  };
  return { all, holding: (name) => all().filter((s) => names(s).includes(name)), multi: () => isMulti(store), nets: () => store.nets };
}

export function useDomainActions(store: Store, deps: { api?: ApiFn } = { api: coreApi as ApiFn }): UseDomainActions {
  const ui = reactive(emptyUi());
  const actions = createDomainActions(ui, deps.api || (coreApi as ApiFn), targetsOf(store));
  watch(() => rowsBusy(mergedDomains(siteNets(store), store)), actions.onRowsBusy);
  return { ui, ...actions };
}
