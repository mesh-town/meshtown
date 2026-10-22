/* Ring texts: the center, the legend, the arc tooltip. */

import { ROLE_ORDER } from "./ring-config.ts";
import type { RingRole } from "./ring-config.ts";
import type { RingArc } from "./ring-arcs.ts";
import type { RingMember } from "./ring-members.ts";
import {
  HOME_LINK_OFFLINE, HOME_LINK_ONLINE, HOME_RING_ALONE, HOME_RING_NO_NET, HOME_RING_SINGLE_CLIENT, HOME_RING_SINGLE_VIA,
  ringClientGroupText, ringCenterMainText, ringOfflineGroupText, ringOnlineGroupText, ringViaGroupText, ringViaLegendText,
} from "./home-flow-texts.ts";

export interface RingCenter {
  main: string;
  sub: string;
}
export interface RingLegendItem {
  role: RingRole;
  text: string;
}

export function ringCenter(members: RingMember[], level: "green" | "red" | "grey"): RingCenter {
  if (members.length > 0) return { main: ringCenterMainText(members.filter((m) => m.role !== "offline").length, members.length), sub: HOME_LINK_ONLINE };
  return { main: level === "green" ? HOME_RING_ALONE : HOME_RING_NO_NET, sub: "" };
}

/** Bucket of the center line length: s — up to 6 chars ("2 of 2"), m — 7–8, l — 9 and more ("999 of 999"). */
export function ringLenBucket(main: string): "s" | "m" | "l" {
  if (main.length <= 6) return "s";
  return main.length <= 8 ? "m" : "l";
}

const GROUP_TEXT: Record<RingRole, (count: number, hasVia: boolean) => string> = {
  via: ringViaGroupText,
  client: ringClientGroupText,
  online: ringOnlineGroupText,
  offline: ringOfflineGroupText,
};

const legendOf = (role: RingRole, list: RingMember[], hasVia: boolean): string =>
  role === "via" ? ringViaLegendText(list[0].label) : GROUP_TEXT[role](list.length, hasVia);

export function ringLegend(members: RingMember[]): RingLegendItem[] {
  const hasVia = members.some((m) => m.role === "via");
  return ROLE_ORDER.map((role) => ({ role, list: members.filter((m) => m.role === role) }))
    .filter((g) => g.list.length > 0)
    .map((g) => ({ role: g.role, text: legendOf(g.role, g.list, hasVia) }));
}

const SINGLE_TEXT: Record<RingRole, string> = {
  via: HOME_RING_SINGLE_VIA,
  client: HOME_RING_SINGLE_CLIENT,
  online: HOME_LINK_ONLINE,
  offline: HOME_LINK_OFFLINE,
};

// `hasVia` — the ring has an exit member, so the online group reads "Also online".
export function arcTitle(arc: RingArc, hasVia = false): string {
  if (arc.members.length === 1) return `${arc.members[0].label} — ${SINGLE_TEXT[arc.role]}`;
  return GROUP_TEXT[arc.role](arc.members.length, hasVia);
}
