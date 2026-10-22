import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import DomainsSection from "./DomainsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const DOMAINS = [
  { name: "example.org", checks: { aaaa1111: { verdict: "network_blocked" }, bbbb2222: { verdict: "ok" } } },
  { name: "fresh.test", checks: {} },
];
let mounted;
const mountDomains = (domains = DOMAINS) => (mounted = mountSection(DomainsSection, { state: baseState({ domains }) }));

afterEach(() => mounted.wrapper.unmount());

describe("DomainsSection", () => {
  it("no state yet — \"Loading…\"", () => {
    mounted = mountSection(DomainsSection, { state: null });
    expect(mounted.wrapper.get("#domains-loading").text()).toBe("Loading…");
    expect(mounted.wrapper.find("table").exists()).toBe(false);
  });

  it("\"Site checks\" table: a row per site, marks by column", () => {
    const { wrapper } = mountDomains();
    const table = wrapper.get('table[aria-label="Site checks"]');
    const rows = table.findAll("tbody tr");
    expect(rows).toHaveLength(2);
    expect(rows[0].text()).toContain("example.org");
    expect(rows[0].text()).toContain("blocked by ISP");
    expect(rows[0].text()).toContain("opens");
    expect(rows[1].text()).toContain("fresh.test");
  });

  it("sites are blocked for you but open for a member — the \"Exit via …\" button", () => {
    const { wrapper } = mountDomains();
    expect(wrapper.get("tbody").text()).toContain("Exit via peer-one");
  });

  it("legend — a list: badge = verdict text, seven items", () => {
    const { wrapper } = mountDomains();
    const items = wrapper.get("#domains-legend").findAll("li");
    expect(items).toHaveLength(7);
    expect(items[0].get(".badge").text()).toBe("opens");
    expect(items[2].text()).toContain("the ISP doesn't let you reach the site");
  });

  it("empty table — the hint \"No sites yet …\"", () => {
    const { wrapper } = mountDomains([]);
    expect(wrapper.get("tbody").text()).toContain("No sites yet — enter an address above and click \"Add\"");
  });

  it("empty field + \"Add\" — no request, no error", async () => {
    const { wrapper } = mountDomains();
    await wrapper.get("#domain-add-btn").trigger("click");
    await flushPromises();
    expect(api).not.toHaveBeenCalled();
    expect(wrapper.get("[role=alert]").text()).toBe("");
  });

  it("node refusal on add — the text in role=alert, the field is not cleared", async () => {
    api.mockRejectedValue(new ApiError(422, "invalid", "This address is not suitable"));
    const { wrapper } = mountDomains();
    await wrapper.get("#domain-add-input").setValue("bad host");
    await wrapper.get("#domain-add-btn").trigger("click");
    await flushPromises();
    expect(api).toHaveBeenCalledWith("POST", "/api/domains", { domain: "bad host" });
    expect(wrapper.get("[role=alert]").text()).toBe("Couldn't add the site: this address is not suitable");
    expect(wrapper.get("#domain-add-input").element.value).toBe("bad host");
  });

  it("successful add clears the field", async () => {
    api.mockResolvedValue({});
    const { wrapper } = mountDomains();
    await wrapper.get("#domain-add-input").setValue("new.example");
    await wrapper.get("#domain-add-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#domain-add-input").element.value).toBe("");
  });
});
