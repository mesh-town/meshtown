// "The members turned this computer down" that has held for `ADMISSION_REFUSED_DELAY_MS` without a break: right after
// joining, "0 online" is normal, so a fresh refusal is not shown yet. Used by the plate and by the header line.

import { computed, onUnmounted, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { Refused } from "./admission-refused.ts";
import { ADMISSION_REFUSED_DELAY_MS } from "./peers-texts.ts";

export function useHeldRefusal(read: () => Refused | null): ComputedRef<Refused | null> {
  const refused = computed(read);
  const held = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  const stop = (): void => {
    if (timer) clearTimeout(timer);
    timer = null;
  };
  watch(() => refused.value !== null, (now) => {
    stop();
    held.value = false;
    if (now) timer = setTimeout(() => (held.value = true), ADMISSION_REFUSED_DELAY_MS);
  }, { immediate: true, flush: "sync" });
  onUnmounted(stop);
  return computed(() => (held.value ? refused.value : null));
}
