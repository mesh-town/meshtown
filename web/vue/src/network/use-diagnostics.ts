/* Wrapper of the "Diagnostics" section core for the section: a reactive form, the network key, computed
 * views and a watcher on the network. The logic is in `diag-*.js`; here only `reactive`/`watch`.
 * The network key lives in `networkKeyState` (a component ref) — not in the store and not in the form. */

import { computed, onUnmounted, reactive, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { NetworkInfo } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import { diagnosticChecks } from "./diag-checks.ts";
import type { CheckRow } from "./diag-checks.ts";
import { aboutLines, eventRows, transportRows } from "./diag-details.ts";
import type { EventRow, TransportRow } from "./diag-details.ts";
import {
  addPeerAddresses, clearDiagTimers, copySelfAddresses, copySummary, createDiagForm, createKeyState,
  defaultDiagDeps, resetKeyIfNetChanged, toggleKey,
} from "./diag-actions.ts";
import type { DiagDeps, DiagForm, KeyResult, KeyState, PeerAddResult, DiagCopyResult } from "./diag-actions.ts";

export interface Diagnostics {
  form: DiagForm;
  networkKeyState: Ref<KeyState>;
  keyText: ComputedRef<string>;
  checks: ComputedRef<CheckRow[]>;
  events: ComputedRef<EventRow[]>;
  transport: ComputedRef<TransportRow[]>;
  about: ComputedRef<string[]>;
  copySummary: () => Promise<DiagCopyResult>;
  copySelfAddresses: () => Promise<DiagCopyResult>;
  addPeerAddresses: () => Promise<PeerAddResult>;
  toggleKey: () => Promise<KeyResult>;
}

const netIdOf = (store: Store): string | null => (store.state && store.state.network && store.state.network.id_hex) || null;

// The full key while it is shown; otherwise the network mask (or empty without a network).
function keyTextOf(key: KeyState, net: NetworkInfo | null | undefined): string {
  if (key.shown && key.full) return key.full;
  return net ? net.masked : "";
}

export function useDiagnostics(store: Store, deps: DiagDeps = defaultDiagDeps): Diagnostics {
  const form = reactive(createDiagForm());
  const networkKeyState = ref<KeyState>(createKeyState());
  const checks = computed(() => diagnosticChecks(store.state));
  const events = computed(() => eventRows(store.state && store.state.log && store.state.log.tail));
  const transport = computed(() => transportRows(store.state && store.state.transport));
  const keyText = computed(() => keyTextOf(networkKeyState.value, store.state && store.state.network));
  const about = computed(() => aboutLines(store.state && store.state.node));

  watch(() => netIdOf(store), (netId) => resetKeyIfNetChanged(networkKeyState.value, netId), { immediate: true });
  onUnmounted(() => {
    resetKeyIfNetChanged(networkKeyState.value, null);
    clearDiagTimers(form, deps);
  });

  return {
    form, networkKeyState, keyText, checks, events, transport, about,
    copySummary: () => copySummary(form, store, deps),
    copySelfAddresses: () => copySelfAddresses(form, store, deps),
    addPeerAddresses: () => addPeerAddresses(form, deps),
    toggleKey: () => toggleKey(networkKeyState.value, store, deps),
  };
}
