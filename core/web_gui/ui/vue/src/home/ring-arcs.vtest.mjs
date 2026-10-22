/* Ring arc geometry: the "per member" and "by role" modes, gaps, minimums. */

import { describe, expect, it } from "vitest";
import { ringArcs } from "./ring-arcs.ts";
import { RING } from "./ring-config.ts";

const TAU = 2 * Math.PI;

describe("ringArcs", () => {
  describe("empty and single set", () => {
    it("no members, no arcs", () => {
      expect(ringArcs([], 112, 14)).toEqual([]);
    });

    it("one member — one arc over the whole circle without a gap", () => {
      const [a, ...rest] = ringArcs(many({ online: 1 }), 112, 14);
      expect(rest).toHaveLength(0);
      expect(a.startRad).toBeCloseTo(0, 6);
      expect(a.endRad).toBeCloseTo(TAU, 6);
      expect(a.lengthPx).toBeCloseTo(TAU * 49, 4);
    });
  });

  describe("one arc per member, 112/14", () => {
    it("three members — three arcs of length 2π·49/3 − 2", () => {
      const arcs = ringArcs(many({ online: 3 }), 112, 14);
      expect(arcs).toHaveLength(3);
      for (const a of arcs) expect(a.lengthPx).toBeCloseTo((TAU * 49) / 3 - RING.gapPx, 4);
    });

    it("eight members — lengths ≈ 38.5 − 2", () => {
      const arcs = ringArcs(many({ online: 8 }), 112, 14);
      expect(arcs).toHaveLength(8);
      for (const a of arcs) expect(a.lengthPx).toBeCloseTo(36.48, 1);
    });

    it("one member per arc, the order as in the input", () => {
      const input = many({ via: 1, client: 1, online: 1, offline: 1 });
      const arcs = ringArcs(input, 112, 14);
      expect(arcs.map((a) => a.members)).toEqual(input.map((m) => [m]));
      expect(arcs.map((a) => a.role)).toEqual(["via", "client", "online", "offline"]);
    });

    it("arc length equals (end − start)·r", () => {
      for (const a of ringArcs(many({ online: 5 }), 112, 14)) {
        expect(a.lengthPx).toBeCloseTo((a.endRad - a.startRad) * 49, 6);
      }
    });

    it("angles go clockwise from 0, do not go beyond 0…2π and do not overlap", () => {
      const arcs = ringArcs(many({ online: 6 }), 112, 14);
      expect(arcs[0].startRad).toBeGreaterThanOrEqual(0);
      expect(arcs[0].startRad).toBeLessThan(TAU / 6);
      expect(arcs.at(-1).endRad).toBeLessThanOrEqual(TAU + 1e-9);
      arcs.slice(1).forEach((a, i) => expect(a.startRad).toBeGreaterThan(arcs[i].endRad));
    });

    it("the gap between neighbouring arcs, and between the last and the first, equals gapPx", () => {
      const arcs = ringArcs(many({ online: 6 }), 112, 14);
      const gapRad = RING.gapPx / 49;
      arcs.slice(1).forEach((a, i) => expect(a.startRad - arcs[i].endRad).toBeCloseTo(gapRad, 6));
      expect(arcs[0].startRad + TAU - arcs.at(-1).endRad).toBeCloseTo(gapRad, 6);
    });

    it("two of the same role — still two arcs with gaps", () => {
      const arcs = ringArcs(many({ online: 2 }), 112, 14);
      expect(arcs).toHaveLength(2);
      expect(arcs[0].lengthPx).toBeCloseTo((TAU * 49) / 2 - RING.gapPx, 4);
    });
  });

  describe("mode threshold", () => {
    it("64/8: 21 members — one arc per member", () => {
      expect(ringArcs(many({ online: 21 }), 64, 8)).toHaveLength(21);
    });

    // 2π·28/22 = 7.997 < 8 — by the formula `2π·r / n ≥ minArcPx` at 22 members it is already the
    // "by role" mode; the test follows the formula.
    it("64/8: 22 members — already arcs by role (2π·28/22 < 8)", () => {
      const arcs = ringArcs(many({ online: 11, offline: 11 }), 64, 8);
      expect(arcs).toHaveLength(2);
    });

    it("64/8: 23 members — arcs by role", () => {
      expect(ringArcs(many({ online: 12, offline: 11 }), 64, 8)).toHaveLength(2);
    });

    it("112/14: 38 members — one each, 39 — by role", () => {
      expect(ringArcs(many({ online: 19, offline: 19 }), 112, 14)).toHaveLength(38);
      expect(ringArcs(many({ online: 19, offline: 20 }), 112, 14)).toHaveLength(2);
    });
  });

  describe("arcs by role, 40 members at 112/14", () => {
    const input = many({ via: 1, client: 5, online: 28, offline: 6 });

    it("one arc per role, in the order via, client, online, offline", () => {
      const arcs = ringArcs(input, 112, 14);
      expect(arcs.map((a) => a.role)).toEqual(["via", "client", "online", "offline"]);
      expect(arcs.map((a) => a.members.length)).toEqual([1, 5, 28, 6]);
    });

    it("the sum of lengths and gaps equals the circumference 2π·r", () => {
      const arcs = ringArcs(input, 112, 14);
      const sum = arcs.reduce((s, a) => s + a.lengthPx, 0) + RING.gapPx * arcs.length;
      expect(sum).toBeCloseTo(TAU * 49, 4);
    });

    it("via is shorter than its share, but not shorter than keepRolePx", () => {
      const [via, client] = ringArcs(input, 112, 14);
      expect(via.lengthPx).toBeGreaterThanOrEqual(RING.keepRolePx - 1e-9);
      expect(client.lengthPx).toBeGreaterThanOrEqual(RING.keepRolePx - 1e-9);
    });

    it("the via shortfall is taken from the others: online to offline as 28 to 6", () => {
      const [, , online, offline] = ringArcs(input, 112, 14);
      expect(online.lengthPx / offline.lengthPx).toBeCloseTo(28 / 6, 4);
    });

    it("arc length in angles: (end − start)·r", () => {
      for (const a of ringArcs(input, 112, 14)) expect(a.lengthPx).toBeCloseTo((a.endRad - a.startRad) * 49, 6);
    });

    it("both via and client are short — both are raised to 10 px, the sum adds up", () => {
      const arcs = ringArcs(many({ via: 1, client: 1, online: 30, offline: 8 }), 112, 14);
      expect(arcs[0].lengthPx).toBeGreaterThanOrEqual(RING.keepRolePx - 1e-9);
      expect(arcs[1].lengthPx).toBeGreaterThanOrEqual(RING.keepRolePx - 1e-9);
      const sum = arcs.reduce((s, a) => s + a.lengthPx, 0) + RING.gapPx * arcs.length;
      expect(sum).toBeCloseTo(TAU * 49, 4);
    });

    it("angles go clockwise from 0 and cover the circle", () => {
      const arcs = ringArcs(input, 112, 14);
      expect(arcs[0].startRad).toBeGreaterThanOrEqual(0);
      expect(arcs.at(-1).endRad).toBeLessThanOrEqual(TAU + 1e-9);
      arcs.slice(1).forEach((a, i) => expect(a.startRad).toBeGreaterThan(arcs[i].endRad));
    });

    it("the gap between roles equals gapPx", () => {
      const arcs = ringArcs(input, 112, 14);
      arcs.slice(1).forEach((a, i) => expect((a.startRad - arcs[i].endRad) * 49).toBeCloseTo(RING.gapPx, 4));
    });
  });

  describe("one role with 40 members", () => {
    it("one arc 0…2π without gaps", () => {
      const arcs = ringArcs(many({ online: 40 }), 112, 14);
      expect(arcs).toHaveLength(1);
      expect(arcs[0].role).toBe("online");
      expect(arcs[0].members).toHaveLength(40);
      expect(arcs[0].startRad).toBeCloseTo(0, 6);
      expect(arcs[0].endRad).toBeCloseTo(TAU, 6);
      expect(arcs[0].lengthPx).toBeCloseTo(TAU * 49, 4);
    });
  });
});

/** Members by role: `many({ via: 1, online: 3 })` — in the ring's role order. */
function many(counts) {
  return ["via", "client", "online", "offline"].flatMap((role) =>
    Array.from({ length: counts[role] ?? 0 }, (_, i) => ({ role, label: `${role}-${i}`, idFull: `${role}-${i}_full` })),
  );
}
