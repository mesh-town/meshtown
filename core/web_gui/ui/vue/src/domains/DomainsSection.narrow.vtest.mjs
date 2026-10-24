/* "Site checks" on a narrow screen: a list of cards instead of the table; the wide screen keeps the table. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { saveExitIn } from "../core/exit-via.ts";
import { PEER_FULL, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { stubNarrow } from "../testing/narrow-media.mjs";
import DomainsSection from "./DomainsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));
vi.mock("../core/exit-via.ts", () => ({ saveExitIn: vi.fn(async () => ({ text: "Saved", cls: "saved" })) }));

// example.org: blocked for you, opens at peer-one (an online exit) — the "Use as exit" case; fresh.test: never checked.
const DOMAINS = [
  { name: "example.org", checks: { aaaa1111: { verdict: "network_blocked", reason: "TCP: reset", age_sec: 5 }, bbbb2222: { verdict: "ok" } } },
  { name: "fresh.test", checks: {} },
];
let mounted;

afterEach(() => mounted.wrapper.unmount());

describe("site checks", () => {
  describe("on a narrow screen", () => {
    describe("with sites", () => {
      it("a card list replaces the table", () => {
        const { wrapper } = mountNarrow();
        expect(wrapper.find(".table-scroll").exists()).toBe(false);
        expect(wrapper.find("table").exists()).toBe(false);
        expect(wrapper.get("div.domain-cards").findAll("div.domain-card")).toHaveLength(2);
      });

      it("the card head has the site name and the \"Check again\" and \"Remove\" text buttons", () => {
        const card = cards(mountNarrow().wrapper)[0];
        expect(card.get(".mono").text()).toContain("example.org");
        expect(buttonTexts(card)).toEqual(expect.arrayContaining(["Check again", "Remove"]));
      });

      it("rows: \"For you\" first, then one row per member by name", () => {
        const card = cards(mountNarrow().wrapper)[0];
        const text = card.text();
        expect(text).toContain("For you");
        expect(text).toContain("peer-one");
        expect(text).toContain("member cccc3333");
        expect(text.indexOf("For you")).toBeLessThan(text.indexOf("peer-one"));
      });

      it("verdict marks are badges with the table's classes and hints", () => {
        const card = cards(mountNarrow().wrapper)[0];
        const badges = card.findAll(".badge");
        const blocked = badges.find((b) => b.text() === "blocked by ISP");
        expect(blocked.classes()).toContain("badge-cut");
        expect(blocked.attributes("title")).toBe("TCP: reset · checked just now");
        expect(badges.find((b) => b.text() === "opens").classes()).toContain("badge-ok");
      });

      it("a site that was not checked says \"not checked yet\"", () => {
        const card = cards(mountNarrow().wrapper)[1];
        expect(card.text()).toContain("not checked yet");
        expect(card.findAll(".badge-ok, .badge-cut")).toHaveLength(0);
      });
    });

    describe("blocked for you, opens at a member", () => {
      it("that member's row has a \"Use as exit\" button with the explanation as its hint", () => {
        const card = cards(mountNarrow().wrapper)[0];
        const use = card.findAll("button").filter((b) => b.text() === "Use as exit");
        expect(use).toHaveLength(1);
        expect(use[0].attributes("title")).toBe("All sites will go through peer-one");
        expect(use[0].element.closest("div").textContent).toContain("peer-one");
      });

      it("the other site has no such button", () => {
        const card = cards(mountNarrow().wrapper)[1];
        expect(buttonTexts(card)).not.toContain("Use as exit");
      });

      it("a click makes this member the only exit", async () => {
        const { wrapper, store } = mountNarrow();
        await cards(wrapper)[0].findAll("button").find((b) => b.text() === "Use as exit").trigger("click");
        await flushPromises();
        expect(saveExitIn).toHaveBeenCalledWith(store, "main", PEER_FULL);
        expect(wrapper.get("#domains-exit-status").text()).toBe("Saved");
      });

      it("the member is already the exit — \"✓ Your exit\" instead of the button", () => {
        const { wrapper } = mountNarrow({ config: baseConfig({ route: { exits: [PEER_FULL] } }) });
        const card = cards(wrapper)[0];
        expect(card.text()).toContain("✓ Your exit");
        expect(buttonTexts(card)).not.toContain("Use as exit");
      });
    });

    describe("actions", () => {
      it("\"Check again\" rechecks that site", async () => {
        const { wrapper } = mountNarrow();
        await buttonByText(cards(wrapper)[1], "Check again").trigger("click");
        await flushPromises();
        expect(api).toHaveBeenCalledWith("POST", "/api/actions/check-domain", { domain: "fresh.test" });
      });

      it("\"Remove\" removes that site", async () => {
        const { wrapper } = mountNarrow();
        await buttonByText(cards(wrapper)[0], "Remove").trigger("click");
        await flushPromises();
        expect(api).toHaveBeenCalledWith("DELETE", "/api/domains?name=example.org");
      });

      it("a site being checked shows \"checking…\" and its two buttons are disabled", () => {
        const domains = [{ ...DOMAINS[1], checking: true }];
        const card = cards(mountNarrow({ domains }).wrapper)[0];
        expect(card.text()).toContain("checking…");
        expect(buttonByText(card, "Check again").attributes("disabled")).toBeDefined();
        expect(buttonByText(card, "Remove").attributes("disabled")).toBeDefined();
      });

      it("a failed action shows the error text inside its card", async () => {
        const { wrapper } = mountNarrow();
        api.mockRejectedValue(new Error("boom"));
        await buttonByText(cards(wrapper)[1], "Remove").trigger("click");
        await flushPromises();
        expect(cards(wrapper)[1].get(".field-error-text").text()).not.toBe("");
        expect(cards(wrapper)[0].find(".field-error-text").exists()).toBe(false);
      });
    });

    describe("without sites", () => {
      it("the same hint as in the table, and no cards", () => {
        const { wrapper } = mountNarrow({ domains: [] });
        expect(wrapper.findAll("div.domain-card")).toHaveLength(0);
        expect(wrapper.text()).toContain("No sites yet — enter an address above and click \"Add\"");
      });
    });

    describe("the rest of the section", () => {
      it("the add row, \"Check all\" and the legend stay", () => {
        const { wrapper } = mountNarrow();
        expect(wrapper.find("#domain-add-input").exists()).toBe(true);
        expect(wrapper.find("#domain-add-btn").exists()).toBe(true);
        expect(wrapper.find("#domains-check-all-btn").exists()).toBe(true);
        expect(wrapper.get("#domains-legend").findAll("li")).toHaveLength(7);
      });
    });
  });

  describe("on a wide screen", () => {
    it("the table stays and there are no cards", () => {
      const { wrapper } = mountNarrow({}, false);
      expect(wrapper.find('table[aria-label="Site checks"]').exists()).toBe(true);
      expect(wrapper.find("div.domain-cards").exists()).toBe(false);
    });

    it("the row buttons keep ⟳ and × with the hints \"Check again\" and \"Remove\" and the old accessible names", () => {
      const { wrapper } = mountNarrow({}, false);
      const recheck = wrapper.get('table button[aria-label="Recheck example.org"]');
      expect(recheck.text()).toBe("⟳");
      expect(recheck.attributes("title")).toBe("Check again");
      const remove = wrapper.get('table button[aria-label="Remove example.org"]');
      expect(remove.text()).toBe("×");
      expect(remove.attributes("title")).toBe("Remove");
    });
  });

  describe("the window is resized", () => {
    it("shrinking switches the table to cards", async () => {
      const { wrapper, media } = mountNarrow({}, false);
      media.change(true);
      await flushPromises();
      expect(wrapper.find("table").exists()).toBe(false);
      expect(wrapper.findAll("div.domain-card")).toHaveLength(2);
    });
  });
});

function mountNarrow({ domains = DOMAINS, config } = {}, narrow = true) {
  const media = stubNarrow(narrow);
  api.mockResolvedValue({});
  mounted = mountSection(DomainsSection, { state: baseState({ domains }), ...(config ? { config } : {}) });
  return { ...mounted, media };
}

const cards = (wrapper) => wrapper.findAll("div.domain-card");
const buttonTexts = (card) => card.findAll("button").map((b) => b.text());
const buttonByText = (card, text) => card.findAll("button").find((b) => b.text() === text);
