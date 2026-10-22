/* Tab icon: the same member ring, 16×16, no text; arcs shorter than 4 px fold into roles. */

import { ringArcs } from "./ring-arcs.ts";
import type { RingArc } from "./ring-arcs.ts";
import type { RingRole } from "./ring-config.ts";
import type { RingMember } from "./ring-members.ts";

const SIZE = 16;
const THICK = 3.5;
const R = (SIZE - THICK) / 2;
const C = SIZE / 2;
const OPTS = { minArcPx: 4, keepRolePx: 4 };

type Palette = Record<RingRole, string>;
const LIGHT: Palette = { via: "#2f6fd0", client: "#af4bcd", online: "#1f8a4c", offline: "#8c8c8c" };
const DARK: Palette = { via: "#5aaaff", client: "#af4bcd", online: "#4fbd80", offline: "#6b7780" };

const num = (v: number): string => String(Math.round(v * 100) / 100);
const point = (rad: number): string => `${num(C + R * Math.sin(rad))} ${num(C - R * Math.cos(rad))}`;

function pathD(arc: RingArc): string {
  const sweep = arc.endRad - arc.startRad;
  if (sweep >= Math.PI * 2 - 1e-6) return `M${C} ${C - R}A${R} ${R} 0 1 1 ${C} ${C + R}A${R} ${R} 0 1 1 ${C} ${C - R}`;
  return `M${point(arc.startRad)}A${R} ${R} 0 ${sweep > Math.PI ? 1 : 0} 1 ${point(arc.endRad)}`;
}

const stroke = (d: string, color: string): string => `<path d="${d}" stroke="${color}"/>`;

export function faviconSvg(members: RingMember[], dark: boolean): string {
  const colors = dark ? DARK : LIGHT;
  const arcs = ringArcs(members, SIZE, THICK, OPTS);
  const paths = arcs.length === 0 ? [stroke(pathD({ role: "online", members: [], startRad: 0, endRad: Math.PI * 2, lengthPx: 0 }), colors.online)] : arcs.map((a) => stroke(pathD(a), colors[a.role]));
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${SIZE} ${SIZE}" width="${SIZE}" height="${SIZE}" fill="none" stroke-width="${THICK}">${paths.join("")}</svg>`;
}

export function faviconHref(svg: string): string {
  return "data:image/svg+xml," + encodeURIComponent(svg);
}
