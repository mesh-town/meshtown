/* Ring geometry: arc angles from 12 o'clock clockwise. The "per member" or "arcs by role" mode. */

import { RING, ROLE_ORDER } from "./ring-config.ts";
import type { RingRole } from "./ring-config.ts";
import type { RingMember } from "./ring-members.ts";

export interface RingArc {
  role: RingRole;
  members: RingMember[];
  startRad: number;
  endRad: number;
  lengthPx: number;
}

const TAU = Math.PI * 2;
const KEEP_ROLES: RingRole[] = ["via", "client"];

interface Slice {
  role: RingRole;
  members: RingMember[];
  lengthPx: number;
}

// Arcs in a row around the circle; between neighbours a gap, split equally on both sides.
function layout(slices: Slice[], r: number): RingArc[] {
  const gapRad = RING.gapPx / r;
  let cursor = 0;
  return slices.map((s) => {
    const startRad = cursor + gapRad / 2;
    const endRad = startRad + s.lengthPx / r;
    cursor = endRad + gapRad / 2;
    return { role: s.role, members: s.members, startRad, endRad, lengthPx: s.lengthPx };
  });
}

function whole(role: RingRole, members: RingMember[], r: number): RingArc[] {
  return [{ role, members, startRad: 0, endRad: TAU, lengthPx: TAU * r }];
}

function perMember(members: RingMember[], r: number): RingArc[] {
  const each = (TAU * r) / members.length - RING.gapPx;
  return layout(members.map((m) => ({ role: m.role, members: [m], lengthPx: each })), r);
}

// Lengths by shares; "via"/"client" are raised to the minimum, the shortfall is taken from the others.
function roleLengths(counts: number[], roles: RingRole[], free: number, keep: number): number[] {
  const total = counts.reduce((a, b) => a + b, 0);
  const raw = counts.map((c) => (free * c) / total);
  const short = roles.map((role, i) => KEEP_ROLES.includes(role) && raw[i] < keep);
  const deficit = raw.reduce((sum, len, i) => sum + (short[i] ? keep - len : 0), 0);
  const restTotal = raw.reduce((sum, len, i) => sum + (short[i] ? 0 : len), 0);
  if (deficit === 0) return raw;
  if (restTotal === 0) return raw.map((len) => len - deficit / raw.length);
  return raw.map((len, i) => (short[i] ? keep : len - (deficit * len) / restTotal));
}

function byRoles(members: RingMember[], r: number, keep: number): RingArc[] {
  const groups = ROLE_ORDER.map((role) => ({ role, list: members.filter((m) => m.role === role) })).filter((g) => g.list.length > 0);
  if (groups.length === 1) return whole(groups[0].role, groups[0].list, r);
  const free = TAU * r - RING.gapPx * groups.length;
  const lengths = roleLengths(groups.map((g) => g.list.length), groups.map((g) => g.role), free, keep);
  return layout(groups.map((g, i) => ({ role: g.role, members: g.list, lengthPx: lengths[i] })), r);
}

interface ArcOpts {
  minArcPx?: number;
  keepRolePx?: number;
}

export function ringArcs(members: RingMember[], size: number, thickness: number, opts?: ArcOpts): RingArc[] {
  const r = (size - thickness) / 2;
  if (members.length === 0) return [];
  if (members.length === 1) return whole(members[0].role, members, r);
  if ((TAU * r) / members.length >= (opts?.minArcPx ?? RING.minArcPx)) return perMember(members, r);
  return byRoles(members, r, opts?.keepRolePx ?? RING.keepRolePx);
}
