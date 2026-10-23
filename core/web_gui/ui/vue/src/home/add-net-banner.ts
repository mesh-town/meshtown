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

// The slot a join is waiting on (set when the node answers "waiting"): it counts as arrived once it holds its key, even
// if no poll saw it as joining.
const expected = ref<string | null>(null);

/** Remembers the slot an add form waits on; `null` forgets it. */
export function expectArrival(slot: string | null): void {
  expected.value = slot;
}

// A network that was being joined (waiting for a member) and now holds its key.
export function arrivedSlots(before: NetBrief[], now: NetBrief[]): string[] {
  const waiting = new Set(before.filter((n) => n.joining && n.id_hex == null).map((n) => n.id));
  return now.filter((n) => waiting.has(n.id) && n.id_hex != null).map((n) => n.id);
}

function arrivals(before: NetBrief[], now: NetBrief[]): string[] {
  const arrived = arrivedSlots(before, now);
  const slot = expected.value;
  if (slot === null || !now.some((n) => n.id === slot && n.id_hex != null)) return arrived;
  expected.value = null;
  return arrived.includes(slot) ? arrived : [...arrived, slot];
}

/** Calls `onArrive(slot)` for each joined network that finished joining while the page was open. */
export function watchArrivals(nets: () => NetBrief[], onArrive: (slot: string) => void): void {
  watch(nets, (now, before) => arrivals(before, now).forEach(onArrive), { flush: "sync" });
}
