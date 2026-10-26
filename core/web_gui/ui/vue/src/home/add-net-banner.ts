/* The one-line banner after a network is added ("You joined…"). The form lives on two pages, the banner on the new
 * network's tab, so the state is module-level. */

import { ref, watch } from "vue";
import type { Ref } from "vue";
import type { NetBrief } from "../core/api-types.ts";

interface NetBanner {
  slot: string;
  text: string;
}

const banner = ref<NetBanner | null>(null);

interface NetBannerApi {
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

/** Somebody else is in the network the banner is about: its text has done its job. */
export function bannerOutlived(banner: NetBanner | null, nets: NetBrief[]): boolean {
  return banner !== null && nets.some((n) => n.id === banner.slot && n.peers_total > 0);
}

/** For tests. */
export function resetNetBanner(): void {
  banner.value = null;
}

// The slot a join is waiting on (set when the node answers "waiting") and the address the user was at: it counts as arrived
// once it holds its key, even if no poll saw it as joining.
const expected = ref<{ slot: string; hash: string } | null>(null);

/** Remembers the slot an add form waits on and where the user is now; `null` forgets it. */
export function expectArrival(slot: string | null): void {
  expected.value = slot === null ? null : { slot, hash: typeof location === "undefined" ? "" : location.hash };
}

/** Forgets the expected slot when it is `slot` (after the page acted on its arrival or refusal). */
export function settleExpected(slot: string): void {
  if (expected.value?.slot === slot) expected.value = null;
}

/** Whether a slot that arrived (or was refused) leads the user to its tab: the user is where the form was sent from, or on the add tab. */
export function leadsToArrival(slot: string, currentHash: string, peersTab: string | null): boolean {
  const waited = expected.value;
  return peersTab === "add" || (waited !== null && waited.slot === slot && waited.hash === currentHash);
}

// A network that was being joined (waiting for a member) and now holds its key.
export function arrivedSlots(before: NetBrief[], now: NetBrief[]): string[] {
  const waiting = new Set(before.filter((n) => n.joining && n.id_hex == null).map((n) => n.id));
  return now.filter((n) => waiting.has(n.id) && n.id_hex != null).map((n) => n.id);
}

function arrivals(before: NetBrief[], now: NetBrief[]): string[] {
  const arrived = arrivedSlots(before, now);
  const slot = expected.value?.slot ?? null;
  if (slot === null || !now.some((n) => n.id === slot && n.id_hex != null)) return arrived;
  return arrived.includes(slot) ? arrived : [...arrived, slot];
}

/** Calls `onArrive(slot)` for each joined network that finished joining while the page was open; the expectation ends after. */
export function watchArrivals(nets: () => NetBrief[], onArrive: (slot: string) => void): void {
  watch(nets, (now, before) => {
    arrivals(before, now).forEach(onArrive);
    const slot = expected.value?.slot;
    if (slot !== undefined && now.some((n) => n.id === slot && n.id_hex != null)) expected.value = null;
  }, { flush: "sync" });
}
