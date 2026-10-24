/* A refusal of the node that the user has already seen: a new attempt hides it until the snapshot changes it
 * (waiting, no join, or another refusal), so the same refusal that comes again is shown again. */

import { computed, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { InviteJoinInfo } from "../core/api-types.ts";

export interface RefusalGate {
  info: ComputedRef<InviteJoinInfo | null | undefined>;
  /** Hide the refusal that is shown now; no refusal - nothing is remembered. */
  dismiss(): void;
}

const keyOf = (info: InviteJoinInfo | null | undefined): string =>
  info?.state === "refused" ? JSON.stringify([info.message, info.label, info.expires_unix]) : "";

export function useRefusalGate(source: () => InviteJoinInfo | null | undefined): RefusalGate {
  const dismissed = ref("");
  // A synchronous watcher: the memory is dropped the moment the snapshot stops showing that refusal.
  watch(() => keyOf(source()), (key) => {
    if (key !== dismissed.value) dismissed.value = "";
  }, { flush: "sync" });
  const info = computed(() => {
    const now = source();
    return dismissed.value && keyOf(now) === dismissed.value ? null : now;
  });
  return { info, dismiss: () => void (dismissed.value = keyOf(source())) };
}
