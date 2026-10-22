/* The connection strip: the text, the token field and "Reload". Paste and reload are events upward. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ConnStrip from "./ConnStrip.vue";

const BAD_TOKEN = { kind: "bad_token", text: "The page was opened from an old or incomplete link — open it again: the Mesh Town icon near the clock → \"Open Mesh Town…\". Or paste the new link:" };

describe("ConnStrip", () => {
  describe("no strip", () => {
    it("hidden and empty", () => {
      const w = mountStrip(null);
      expect(w.get("#conn-strip").attributes("hidden")).toBeDefined();
      expect(w.text()).toBe("");
    });
  });

  describe("live role", () => {
    it("the strip is announced by a screen reader: role=alert", () => {
      expect(mountStrip(null).get("#conn-strip").attributes("role")).toBe("alert");
    });
  });

  describe("regular strip", () => {
    it("shows the text, without a field and buttons", () => {
      const w = mountStrip({ kind: "disconnected", text: "Mesh Town is not responding" });
      expect(w.get("#conn-strip").attributes("hidden")).toBeUndefined();
      expect(w.text()).toBe("Mesh Town is not responding");
      expect(w.find("input").exists()).toBe(false);
      expect(w.find("button").exists()).toBe(false);
    });
  });

  describe("a token is needed", () => {
    it("the text, the address field and \"Open\"; the width via a class, not style", () => {
      const w = mountStrip(BAD_TOKEN);
      expect(w.text()).toContain("The page was opened from an old or incomplete link — open it again: the Mesh Town icon near the clock → \"Open Mesh Town…\". Or paste the new link:");
      expect(w.get("#token-url-apply").text()).toBe("Open");
      const input = w.get("#token-url-input");
      expect(input.classes()).toContain("input-token");
      expect(input.attributes("style")).toBeUndefined();
      expect(input.attributes("placeholder")).toBe("page link");
    });

    it("the pasted address goes out as an apply-token event", async () => {
      const w = mountStrip(BAD_TOKEN);
      await w.get("#token-url-input").setValue("http://127.0.0.1:7070/?token=abc");
      await w.get("#token-url-apply").trigger("click");
      expect(w.emitted("apply-token")).toEqual([["http://127.0.0.1:7070/?token=abc"]]);
    });

    it("after \"Open\" the field is empty: an address with a token does not stay on screen", async () => {
      const w = mountStrip(BAD_TOKEN);
      await w.get("#token-url-input").setValue("http://127.0.0.1:7070/?token=abc");
      await w.get("#token-url-apply").trigger("click");
      expect(w.get("#token-url-input").element.value).toBe("");
    });

    it("the field is not remembered by the browser and not spell-checked", () => {
      const input = mountStrip(BAD_TOKEN).get("#token-url-input");
      expect(input.attributes("autocomplete")).toBe("off");
      expect(input.attributes("spellcheck")).toBe("false");
    });

    it("an empty field — no event", async () => {
      const w = mountStrip(BAD_TOKEN);
      await w.get("#token-url-apply").trigger("click");
      expect(w.emitted("apply-token")).toBeUndefined();
    });
  });

  describe("there is a session", () => {
    it("the \"token needed\" strip is not shown: empty and hidden", () => {
      const w = mount(ConnStrip, { props: { strip: BAD_TOKEN, hasSession: true } });
      expect(w.get("#conn-strip").attributes("hidden")).toBeDefined();
      expect(w.find("#token-url-input").exists()).toBe(false);
    });

    it("other strips stay during a session", () => {
      const w = mount(ConnStrip, { props: { strip: { kind: "disconnected", text: "Mesh Town is not responding" }, hasSession: true } });
      expect(w.get("#conn-strip").attributes("hidden")).toBeUndefined();
      expect(w.text()).toBe("Mesh Town is not responding");
    });
  });

  describe("settings changed in another tab", () => {
    it("\"Reload\" sends reload", async () => {
      const w = mountStrip({ kind: "changed", text: "Settings were changed in another tab or by Mesh Town itself" });
      expect(w.text()).toContain("Settings were changed in another tab or by Mesh Town itself");
      await w.get("#conn-strip-reload").trigger("click");
      expect(w.emitted("reload")).toHaveLength(1);
    });
  });
});

function mountStrip(strip) {
  return mount(ConnStrip, { props: { strip } });
}
