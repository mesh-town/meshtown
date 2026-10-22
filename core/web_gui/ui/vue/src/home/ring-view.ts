/* The ring view for `StatePanel` and the ring size by window width. */

import { onUnmounted, reactive } from "vue";
import type { NodeState } from "../core/api-types.ts";
import type { PanelLevel } from "./home-view.ts";
import { RING } from "./ring-config.ts";
import { ringMembers } from "./ring-members.ts";
import type { RingMember } from "./ring-members.ts";
import { ringCenter, ringLegend } from "./ring-text.ts";
import type { RingCenter, RingLegendItem } from "./ring-text.ts";

export interface RingView {
  members: RingMember[];
  center: RingCenter;
  legend: RingLegendItem[];
}

export function ringView(state: Partial<NodeState> | null, level: PanelLevel): RingView | null {
  if (state === null) return null;
  const members = ringMembers(state);
  return { members, center: ringCenter(members, level), legend: ringLegend(members) };
}

const NARROW_QUERY = "(max-width: 599px)";
const sizeFor = (narrow: boolean): { size: number; thickness: number } => (narrow ? { size: RING.sizeNarrowPx, thickness: RING.thickNarrowPx } : { size: RING.sizeWidePx, thickness: RING.thickWidePx });

/** Ring size: up to 600 px — small. Without `matchMedia` — wide. */
export function useRingSize(): { size: number; thickness: number } {
  const media = typeof matchMedia === "function" ? matchMedia(NARROW_QUERY) : null;
  const dims = reactive(sizeFor(media !== null && media.matches));
  if (media === null) return dims;
  const onChange = (e: MediaQueryListEvent): void => { Object.assign(dims, sizeFor(e.matches)); };
  media.addEventListener?.("change", onChange);
  onUnmounted(() => media.removeEventListener?.("change", onChange));
  return dims;
}
