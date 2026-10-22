/* Wrapper around the `domain-actions.ts` core for the section: reactive state and watching
 * `rowsBusy`. `deps = { api }` can be replaced in a test. */

import { reactive, watch } from "vue";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { api as coreApi } from "../core/api.ts";
import { createDomainActions, emptyUi } from "./domain-actions.ts";
import type { DomainActions, DomainUi } from "./domain-actions.ts";
import { rowsBusy } from "./domains-view.ts";

export type UseDomainActions = { ui: DomainUi } & DomainActions;

export function useDomainActions(store: Store, deps: { api?: ApiFn } = { api: coreApi as ApiFn }): UseDomainActions {
  const ui = reactive(emptyUi());
  const actions = createDomainActions(ui, deps.api || (coreApi as ApiFn));
  watch(() => rowsBusy(store.state && store.state.domains), actions.onRowsBusy);
  return { ui, ...actions };
}
