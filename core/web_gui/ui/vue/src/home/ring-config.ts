/* Parameters of the member ring on Home. Everything that depends on design decisions
 * lives here and in the CSS variables `--ring-*`; nowhere else. */

export const RING = {
  gapPx: 2, // gap between neighbouring arcs
  minArcPx: 8, // shorter — members fold into arcs by role
  keepRolePx: 10, // "your exit" and "through you" are no shorter than this in role mode
  sizeWidePx: 112,
  thickWidePx: 14,
  sizeNarrowPx: 64,
  thickNarrowPx: 8,
} as const;

/** Role order clockwise from 12 o'clock: as in the tray icon. */
export const ROLE_ORDER = ["via", "client", "online", "offline"] as const;
export type RingRole = (typeof ROLE_ORDER)[number];
