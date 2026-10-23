/* The one-line banner after a network is added ("You joined…"). The form lives on two pages, the banner on the new
 * network's tab, so the state is module-level. */

import { ref, watch } from "vue";
import type { Ref } from "vue";
import type { NetBrief } from "../core/api-types.ts";

export interface NetBanner {
  slot: string;
  text: string;
}

const banner = ref<NetBanner | null>(null);

export interface NetBannerApi {
  banner: Ref<NetBanner | null>;
  show: (slot: string, text: string) => void;
  clear: () => void;
}

export function useNetBanner(): NetBannerApi {
  return {
    banner,
    show: (slot, text) => {
      banner.value = { slot, text };
    },
    clear: () => {
      banner.value = null;
    },
  };
}

/** For tests. */
export function resetNetBanner(): void {
  banner.value = null;
}

// A network that was being joined (waiting for a member) and now holds its key.
export function arrivedSlots(before: NetBrief[], now: NetBrief[]): string[] {
  const waiting = new Set(before.filter((n) => n.joining && n.id_hex == null).map((n) => n.id));
  return now.filter((n) => waiting.has(n.id) && n.id_hex != null).map((n) => n.id);
}

/** Calls `onArrive(slot)` for each joined network that finished joining while the page was open. */
export function watchArrivals(nets: () => NetBrief[], onArrive: (slot: string) => void): void {
  watch(nets, (now, before) => arrivedSlots(before, now).forEach(onArrive), { flush: "sync" });
}
