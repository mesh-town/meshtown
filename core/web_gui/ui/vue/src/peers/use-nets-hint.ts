/* The one-time hint shown when the computer is in several networks; "OK" remembers it, going back to a single network forgets it. */

import { ref, watch } from "vue";
import type { Ref } from "vue";
import { PEERS_NETS_HINT_KEY } from "./peers-texts.ts";

interface NetsHint {
  seen: Ref<boolean>;
  dismiss: () => void;
}

export function useNetsHint(count: () => number): NetsHint {
  const seen = ref(localStorage.getItem(PEERS_NETS_HINT_KEY) === "1");
  watch(count, (n) => {
    if (n >= 2) return;
    localStorage.removeItem(PEERS_NETS_HINT_KEY);
    seen.value = false;
  });
  return {
    seen,
    dismiss: () => {
      localStorage.setItem(PEERS_NETS_HINT_KEY, "1");
      seen.value = true;
    },
  };
}
