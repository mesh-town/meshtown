/* The member ring: a path per arc, <title> tooltips, dashes for an empty set, text in the center. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import RingMark from "./RingMark.vue";

describe("RingMark", () => {
  describe("root and svg", () => {
    it("the root is div.ring-mark, inside is an svg with a viewBox by size", () => {
      const w = mountRing(many({ online: 2 }));
      expect(w.element.tagName).toBe("DIV");
      expect(w.classes()).toContain("ring-mark");
      expect(w.get("svg").attributes("viewBox")).toBe("0 0 112 112");
    });

    it("the svg is hidden from the screen reader: aria-hidden=true", () => {
      expect(mountRing(many({ online: 2 })).get("svg").attributes("aria-hidden")).toBe("true");
    });

    it("viewBox follows size", () => {
      expect(mountRing(many({ online: 2 }), { size: 64, thickness: 8 }).get("svg").attributes("viewBox")).toBe("0 0 64 64");
    });
  });

  describe("exit through another member", () => {
    const via = { role: "via", label: "nas", idFull: "n", relayed: true };

    it("dasharray 6 3 wide, 4 2 narrow", () => {
      const wide = mountRing([via, ...many({ online: 1 })]).get("path.ring-via");
      expect(wide.classes()).toContain("ring-relayed");
      expect(wide.attributes("stroke-dasharray")).toBe("6 3");
      const narrow = mountRing([via, ...many({ online: 1 })], { size: 64, thickness: 8 }).get("path.ring-via");
      expect(narrow.attributes("stroke-dasharray")).toBe("4 2");
    });

    it("short arc has no relayed class", () => {
      const w = mountRing([via, ...many({ online: 30 })], { size: 64, thickness: 8 });
      const arc = w.get("path.ring-via");
      expect(arc.classes()).not.toContain("ring-relayed");
      expect(arc.attributes("stroke-dasharray")).toBeUndefined();
    });

    it("an online member through another member is dashed too", () => {
      const on = { role: "online", label: "a", idFull: "a", relayed: true };
      expect(mountRing([on, ...many({ online: 1 })]).get("path.ring-online").attributes("stroke-dasharray")).toBe("6 3");
    });

    it("folded by roles, direct and relayed online members are two arcs", () => {
      const relayed = Array.from({ length: 10 }, (_, i) => ({ role: "online", label: `r${i}`, idFull: `r${i}`, relayed: true }));
      const w = mountRing([...many({ online: 30 }), ...relayed]);
      expect(w.findAll("path.ring-online")).toHaveLength(2);
      expect(w.findAll("path.ring-relayed")).toHaveLength(1);
    });

    it("a direct exit has no dash", () => {
      const w = mountRing([{ ...via, relayed: false }, ...many({ online: 1 })]);
      expect(w.get("path.ring-via").attributes("stroke-dasharray")).toBeUndefined();
    });
  });

  describe("arcs", () => {
    it("three members — three paths", () => {
      expect(mountRing(many({ online: 3 })).findAll("path")).toHaveLength(3);
    });

    it("forty members — paths by the number of roles", () => {
      const w = mountRing(many({ via: 1, client: 5, online: 28, offline: 6 }));
      expect(w.findAll("path")).toHaveLength(4);
    });

    it("path: class ring-arc and ring-<role>, thickness, no fill", () => {
      const w = mountRing(many({ via: 1, offline: 1 }), { thickness: 14 });
      const [via, off] = w.findAll("path");
      expect(via.classes()).toEqual(expect.arrayContaining(["ring-arc", "ring-via"]));
      expect(off.classes()).toEqual(expect.arrayContaining(["ring-arc", "ring-offline"]));
      expect(via.attributes("stroke-width")).toBe("14");
      expect(via.attributes("fill")).toBe("none");
    });

    it("color only by classes: path has no stroke attribute", () => {
      expect(mountRing(many({ online: 2 })).get("path").attributes("stroke")).toBeUndefined();
    });

    it("in the <title> of a single arc — the member's label", () => {
      const w = mountRing([{ role: "via", label: "Dad's PC", idFull: "x" }, ...many({ online: 1 })]);
      expect(w.findAll("path")[0].get("title").text()).toBe("Dad's PC — your exit, online");
    });

    it("in the <title> of a multi-member arc — the count by role", () => {
      const w = mountRing(many({ online: 40 }));
      expect(w.get("path title").text()).toBe("Online: 40");
    });

    it("with a chosen exit among the members — the online arc says \"Also online: N\"", () => {
      const w = mountRing([{ role: "via", label: "Dad's PC", idFull: "x" }, ...many({ online: 40 })]);
      const titles = w.findAll("path title").map((t) => t.text());
      expect(titles).toContain("Also online: 40");
      expect(titles).not.toContain("Online: 40");
    });

    it("the ring is not dashed if there are members", () => {
      expect(mountRing(many({ online: 1 })).find("circle.ring-empty").exists()).toBe(false);
    });
  });

  describe("empty set", () => {
    it("instead of arcs — a dashed circle.ring-empty", () => {
      const w = mountRing([]);
      expect(w.findAll("path")).toHaveLength(0);
      expect(w.findAll("circle.ring-empty")).toHaveLength(1);
    });

    it("and with the center \"only you\" the ring stays dashed", () => {
      const w = mountRing([], { center: { main: "only you", sub: "" } });
      expect(w.find("circle.ring-empty").exists()).toBe(true);
    });
  });

  describe("center", () => {
    it("two spans: the main one and the caption, over the svg", () => {
      const w = mountRing(many({ online: 2 }), { center: { main: "2 of 2", sub: "online" } });
      expect(w.get("span.ring-center-main").text()).toBe("2 of 2");
      expect(w.get("span.ring-center-sub").text()).toBe("online");
      expect(w.get("svg").find(".ring-center-main").exists()).toBe(false);
    });

    it("without center there are no spans", () => {
      const w = mountRing(many({ online: 2 }));
      expect(w.find(".ring-center-main").exists()).toBe(false);
      expect(w.find(".ring-center-sub").exists()).toBe(false);
    });

    it("center = null — no spans", () => {
      expect(mountRing(many({ online: 2 }), { center: null }).find(".ring-center-main").exists()).toBe(false);
    });
  });
  describe("class by the length of the count (CSP: no :style)", () => {
    const withMain = (main) => mountRing(many({ online: 2 }), { center: { main, sub: "online" } });

    it.each([["2 of 2", "s"], ["12 of 40", "m"], ["35 of 400", "l"], ["999 of 999", "l"]])("\"%s\" — ring-len-%s", (main, bucket) => {
      expect(withMain(main).classes()).toContain(`ring-len-${bucket}`);
    });

    it("without a center — the short class s, no inline style", () => {
      const w = mountRing(many({ online: 2 }), { center: null });
      expect(w.classes()).toContain("ring-len-s");
      expect(w.html()).not.toContain("style=");
    });
  });

});

function mountRing(members, extra = {}) {
  return mount(RingMark, { props: { members, size: 112, thickness: 14, ...extra } });
}

function many(counts) {
  return ["via", "client", "online", "offline"].flatMap((role) =>
    Array.from({ length: counts[role] ?? 0 }, (_, i) => ({ role, label: `${role}-${i}`, idFull: `${role}-${i}_full` })),
  );
}
