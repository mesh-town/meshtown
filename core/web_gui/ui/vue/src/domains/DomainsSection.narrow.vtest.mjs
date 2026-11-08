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
const PEER2_EXIT_FULL = "D".repeat(64);
const PEER2 = { id: "dddd4444", id_full: PEER2_EXIT_FULL, name: "peer-two", country: null, version: null, online: true, path: "direct", rtt_ms: 30, exit: true };
const ACTIVE = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, exit: true, country: null, active: true };
// peer-one is the active exit and opens example.org.
const viaState = () => baseState({ domains: DOMAINS, route: { exits: [ACTIVE] } });
// peer-one is the active exit and blocked for example.org, peer-two opens it.
const blockedViaState = () => {
  const domains = [{ name: "example.org", checks: { aaaa1111: { verdict: "network_blocked", age_sec: 5 }, bbbb2222: { verdict: "network_blocked", age_sec: 5 }, dddd4444: { verdict: "ok" } } }, DOMAINS[1]];
  const base = baseState({ domains, route: { exits: [ACTIVE] } });
  return { ...base, peers: [...base.peers, PEER2] };
};
const DOMAINS = [
  { name: "example.org", checks: { aaaa1111: { verdict: "network_blocked", reason: "TCP: reset", age_sec: 5 }, bbbb2222: { verdict: "ok" } } },
  { name: "fresh.test", checks: {} },
];
let mounted;

// peer-one is the active exit and opens example.org, three more exits open it too: "also via 3 more".
const extraPeer = (n) => ({ id: n.repeat(8), id_full: n.toUpperCase().repeat(64), name: "extra-" + n, country: null, version: null, online: true, path: "direct", rtt_ms: 30, exit: true });
const manyState = () => {
  const extra = ["d", "e", "f"].map(extraPeer);
  const checks = Object.fromEntries([["aaaa1111", { verdict: "network_blocked" }], ["bbbb2222", { verdict: "ok" }], ...extra.map((p) => [p.id, { verdict: "ok" }])]);
  const base = baseState({ domains: [{ name: "example.org", checks }], route: { exits: [{ id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, exit: true, country: null, active: true }] } });
  return { ...base, peers: [...base.peers, ...extra] };
};

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

      it("the card has the site name and the \"Details\", \"Check again\" and \"Remove\" text buttons", () => {
        const card = cards(mountNarrow().wrapper)[0];
        expect(card.get(".mono").text()).toContain("example.org");
        expect(buttonTexts(card).slice(-3)).toEqual(["Details", "Check again", "Remove"]);
      });

      it("the result line names the exit: \"Via peer-one: opens · …\"", () => {
        const card = cards(mountNarrow({ state: viaState() }).wrapper)[0];
        expect(card.get(".domain-card-result").text()).toBe("Via peer-one: opens");
      });

      it("without an exit the result line is \"For you: …\" with the own check", () => {
        const card = cards(mountNarrow().wrapper)[0];
        expect(card.get(".domain-card-result").text()).toBe("For you: blocked by provider · just now");
      });

      it("a site that was not checked says \"not checked yet\"", () => {
        const card = cards(mountNarrow().wrapper)[1];
        expect(card.text()).toContain("not checked yet");
        expect(card.findAll(".badge-ok, .badge-cut")).toHaveLength(0);
      });
    });

    describe("the \"N more\" counter", () => {
      it("\"3 more\" opens and closes the same member list as Details", async () => {
        const card = cards(mountNarrow({ state: manyState() }).wrapper)[0];
        const more = buttonByText(card, "3 more");
        const details = buttonByText(card, "Details");
        expect(card.get(".domain-card-others").text()).toContain("also via 3 more");
        expect(more.attributes("aria-expanded")).toBe("false");
        await more.trigger("click");
        expect(card.find(".site-members").exists()).toBe(true);
        expect(more.attributes("aria-expanded")).toBe("true");
        expect(details.attributes("aria-expanded")).toBe("true");
        expect(details.text()).toBe("Hide");
        await more.trigger("click");
        expect(card.find(".site-members").exists()).toBe(false);
      });

      it("no counter — no such button", () => {
        const card = cards(mountNarrow({ state: blockedViaState() }).wrapper)[0];
        expect(card.find(".site-more").exists()).toBe(false);
      });
    });

    describe("details", () => {
      it("\"Details\" opens the member rows and flips aria-expanded", async () => {
        const card = cards(mountNarrow().wrapper)[0];
        const btn = buttonByText(card, "Details");
        expect(btn.attributes("aria-expanded")).toBe("false");
        expect(card.find(".site-members").exists()).toBe(false);
        await btn.trigger("click");
        expect(btn.attributes("aria-expanded")).toBe("true");
        expect(btn.text()).toBe("Hide");
        const text = card.get(".site-members").text();
        expect(text).toContain("This computer, directly");
        expect(text).toContain("peer-one");
        expect(text).not.toContain("member cccc3333");
        expect(text.indexOf("This computer")).toBeLessThan(text.indexOf("peer-one"));
      });
    });

    describe("blocked at your exit, opens at another member", () => {
      it("the card has an \"Exit via peer-two\" button with the explanation as its hint", () => {
        const card = cards(mountNarrow({ state: blockedViaState() }).wrapper)[0];
        const use = card.findAll("button").filter((b) => b.text() === "Exit via peer-two");
        expect(use).toHaveLength(1);
        expect(use[0].attributes("title")).toBe("All sites will go through peer-two");
        expect(card.get(".domain-card-others").text()).toContain("opens via peer-two");
      });

      it("the other site has no such button", () => {
        const card = cards(mountNarrow({ state: blockedViaState() }).wrapper)[1];
        expect(buttonTexts(card).filter((t) => t.startsWith("Exit via"))).toEqual([]);
      });

      it("nobody shares internet — no alternatives line", () => {
        const state = baseState({ domains: DOMAINS, peers: baseState().peers.map((p) => ({ ...p, exit: false })) });
        const card = cards(mountNarrow({ state }).wrapper)[0];
        expect(card.find(".domain-card-others").exists()).toBe(false);
      });

      it("a click makes this member the only exit", async () => {
        const { wrapper, store } = mountNarrow({ state: blockedViaState() });
        await buttonByText(cards(wrapper)[0], "Exit via peer-two").trigger("click");
        await flushPromises();
        expect(saveExitIn).toHaveBeenCalledWith(store, "main", PEER2_EXIT_FULL);
        expect(wrapper.get("#domains-exit-status").text()).toBe("Done \u2014 sites now open via peer-two.");
      });

      it("the member rows in the details say \"Use as exit\" and \"✓ Your exit\"", async () => {
        const { wrapper } = mountNarrow({ state: blockedViaState() });
        const card = cards(wrapper)[0];
        await buttonByText(card, "Details").trigger("click");
        expect(buttonTexts(card)).toContain("Use as exit");
        expect(card.get(".site-members").text()).toContain("✓ Your exit");
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
        expect(wrapper.get("#domains-legend").findAll("li")).toHaveLength(8);
      });
    });
  });

  describe("on a wide screen", () => {
    it("the table stays and there are no cards", () => {
      const { wrapper } = mountNarrow({}, false);
      expect(wrapper.find('table[aria-label="Site checks"]').exists()).toBe(true);
      expect(wrapper.find("div.domain-cards").exists()).toBe(false);
    });

    it("the row buttons keep their svg glyphs with the hints \"Check again\" and \"Remove\" and the old accessible names", () => {
      const { wrapper } = mountNarrow({}, false);
      const recheck = wrapper.get('table button[aria-label="Recheck example.org"]');
      expect(recheck.get("svg").attributes("aria-hidden")).toBe("true");
      expect(recheck.attributes("title")).toBe("Check again");
      const remove = wrapper.get('table button[aria-label="Remove example.org"]');
      expect(remove.get("svg").attributes("aria-hidden")).toBe("true");
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

function mountNarrow({ domains = DOMAINS, config, state } = {}, narrow = true) {
  const media = stubNarrow(narrow);
  api.mockResolvedValue({});
  mounted = mountSection(DomainsSection, { state: state || baseState({ domains }), ...(config ? { config } : {}) });
  return { ...mounted, media };
}

const cards = (wrapper) => wrapper.findAll("div.domain-card");
const buttonTexts = (card) => card.findAll("button").map((b) => b.text());
const buttonByText = (card, text) => card.findAll("button").find((b) => b.text() === text);
