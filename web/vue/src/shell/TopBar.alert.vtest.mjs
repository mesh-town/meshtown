/* The column header when the status line carries `alert: true` (a warning the person must notice, e.g. the chosen exit
 * was removed from the network): the line is `role="alert"` instead of `role="status"`, the rest of the header is the same. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TopBar from "./TopBar.vue";

const WARNING = "Your exit peer-one was removed from the network — sites won't open until you choose another exit";

describe("TopBar", () => {
  describe("the line is an alert", () => {
    it("role=alert, and no role=status next to it", () => {
      const w = mountBar({ level: "red", text: WARNING, alert: true });
      expect(w.get("#status-line").attributes("role")).toBe("alert");
      expect(w.findAll("[role=alert]")).toHaveLength(1);
      expect(w.findAll("[role=status]")).toHaveLength(0);
    });

    it("shows the text with a red dot hidden from screen readers", () => {
      const w = mountBar({ level: "red", text: WARNING, alert: true });
      expect(w.get("#status-line").text()).toBe(WARNING);
      expect(w.get(".dot").classes()).toContain("dot-red");
      expect(w.get(".dot").attributes("aria-hidden")).toBe("true");
    });

    it("the link to the removals that wait for a confirmation still follows the text", () => {
      const w = mount(TopBar, { props: { status: { level: "red", text: WARNING, alert: true }, votes: 1 } });
      expect(w.get("#status-line").attributes("role")).toBe("alert");
      expect(w.get("#strip-vote").text()).toBe("1 request to confirm");
    });
  });

  describe("the line is an ordinary one", () => {
    it("without the alert flag: role=status", () => {
      const w = mountBar({ level: "green", text: "In the browser, sites open directly" });
      expect(w.get("#status-line").attributes("role")).toBe("status");
      expect(w.findAll("[role=alert]")).toHaveLength(0);
    });

    it("alert: false: role=status", () => {
      const w = mountBar({ level: "red", text: "peer-one is offline", alert: false });
      expect(w.get("#status-line").attributes("role")).toBe("status");
      expect(w.findAll("[role=alert]")).toHaveLength(0);
    });
  });

  describe("the same bar gets another line", () => {
    it("the role follows the flag when the line changes", async () => {
      const w = mountBar({ level: "green", text: "In the browser, sites open directly" });
      await w.setProps({ status: { level: "red", text: WARNING, alert: true } });
      expect(w.get("#status-line").attributes("role")).toBe("alert");
      await w.setProps({ status: { level: "green", text: "In the browser, sites open directly" } });
      expect(w.get("#status-line").attributes("role")).toBe("status");
    });
  });
});

function mountBar(status) {
  return mount(TopBar, { props: { status } });
}
