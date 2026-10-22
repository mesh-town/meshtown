/* The column header: the status line with a colored dot (role=status). */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { baseState } from "../testing/section.mjs";
import { createConn } from "./conn.ts";
import { statusLine } from "./status-line.ts";
import TopBar from "./TopBar.vue";

describe("TopBar", () => {
  describe("sites open directly", () => {
    it("role=status with the phrase \"Sites open directly…\" and a green dot", () => {
      const w = mountBar(baseState({ route: { state: "direct" } }));
      expect(w.get("[role=status]").text()).toMatch(/^Sites open directly/);
      expect(w.get(".dot").classes()).toContain("dot-green");
    });
  });

  describe("the selected exit is offline", () => {
    it("red dot", () => {
      const route = { state: "unavailable", exits: [{ id: "bbbb2222", name: "peer-one" }] };
      const w = mountBar(baseState({ route }));
      expect(w.get(".dot").classes()).toContain("dot-red");
      expect(w.get("[role=status]").text()).toContain("offline");
    });
  });

  describe("no snapshot", () => {
    it("\"Mesh Town is not responding\", gray dot", () => {
      const w = mountBar(null);
      expect(w.get("[role=status]").text()).toBe("Mesh Town is not responding");
      expect(w.get(".dot").classes()).toContain("dot-grey");
    });
  });

  describe("line in the header", () => {
    it("inside header, the dot is hidden from screen readers, the text has no \"null\" and \"undefined\"", () => {
      const w = mountBar(baseState({ network: { label: null } }));
      expect(w.element.tagName).toBe("HEADER");
      expect(w.get(".dot").attributes("aria-hidden")).toBe("true");
      expect(w.text()).not.toMatch(/null|undefined/);
    });
  });
});

function mountBar(state) {
  return mount(TopBar, { props: { status: statusLine(state, createConn()) } });
}
