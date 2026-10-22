/* Tab icon: SVG shape, number of arcs, empty set, palettes, href. */

import { describe, expect, it } from "vitest";
import { faviconHref, faviconSvg } from "./ring-favicon.ts";

const paths = (svg) => svg.match(/<path /g)?.length ?? 0;

function many(counts) {
  return ["via", "client", "online", "offline"].flatMap((role) =>
    Array.from({ length: counts[role] ?? 0 }, (_, i) => ({ role, label: `${role}-${i}`, idFull: `${role}-${i}_full` })),
  );
}

describe("faviconSvg", () => {
  it("root 16×16 without text", () => {
    const svg = faviconSvg(many({ online: 3 }), false);
    expect(svg).toMatch(/^<svg [^>]*viewBox="0 0 16 16"/);
    expect(svg).not.toContain("<text");
    expect(svg).toContain("A6.25 6.25");
  });

  it("three members — three paths", () => {
    expect(paths(faviconSvg(many({ online: 3 }), false))).toBe(3);
  });

  it("forty members — arcs by role, no more than four", () => {
    const n = paths(faviconSvg(many({ via: 1, client: 2, online: 30, offline: 7 }), false));
    expect(n).toBeGreaterThan(0);
    expect(n).toBeLessThanOrEqual(4);
  });

  it("empty set — a solid green ring", () => {
    const svg = faviconSvg([], false);
    expect(paths(svg)).toBe(1);
    expect(svg).toContain("#1f8a4c");
  });

  it("light and dark palettes", () => {
    const all = many({ via: 1, client: 1, online: 1, offline: 1 });
    const light = faviconSvg(all, false);
    const dark = faviconSvg(all, true);
    for (const c of ["#2f6fd0", "#af4bcd", "#1f8a4c", "#8c8c8c"]) expect(light).toContain(c);
    for (const c of ["#5aaaff", "#af4bcd", "#4fbd80", "#6b7780"]) expect(dark).toContain(c);
  });
});

describe("faviconHref", () => {
  it("data:image/svg+xml, and escaped markup", () => {
    const href = faviconHref(faviconSvg([], false));
    expect(href.startsWith("data:image/svg+xml,")).toBe(true);
    expect(href).not.toContain("<");
  });
});
