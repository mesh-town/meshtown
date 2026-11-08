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
    expect(rows[0].text()).toContain("opens via peer-one");
    expect(rows[1].text()).toContain("fresh.test");
  });

  it("shows no-exit hint when nobody shares internet", () => {
    const base = baseState({ domains: DOMAINS });
    const withExit = (exit) => {
      mounted = mountSection(DomainsSection, { state: { ...base, peers: base.peers.map((p) => (p.self ? p : { ...p, exit })) } });
      return mounted.wrapper.find("#domains-no-exit").exists();
    };
    expect(withExit(false)).toBe(true);
    mounted.wrapper.unmount();
    expect(withExit(true)).toBe(false);
  });

  it("sites are blocked for you but open for a member — the \"Exit via …\" button", () => {
    const { wrapper } = mountDomains();
    expect(wrapper.get("tbody").text()).toContain("Exit via peer-one");
  });

  it("Details opens the member rows and flips aria-expanded", async () => {
    const { wrapper } = mountDomains();
    const btn = wrapper.findAll("tbody button").find((b) => b.text() === "Details");
    expect(btn.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(".site-members").exists()).toBe(false);
    await btn.trigger("click");
    expect(btn.attributes("aria-expanded")).toBe("true");
    expect(btn.text()).toBe("Hide");
    const text = wrapper.get(".site-members").text();
    expect(text).toContain("This computer, directly");
    expect(text).toContain("peer-one");
    await btn.trigger("click");
    expect(wrapper.find(".site-members").exists()).toBe(false);
  });

  it("header: Site | For you (no exit chosen) | Other exits | actions", () => {
    const { wrapper } = mountDomains();
    const heads = wrapper.findAll('th').map((h) => h.text());
    expect(heads.slice(0, 3)).toEqual(["Site", "For youno exit chosen", "Other exits"]);
  });

  it("empty list — no column headers", () => {
    const { wrapper } = mountDomains([]);
    expect(wrapper.findAll("th")).toHaveLength(0);
    expect(wrapper.find("#domains-groups-row").exists()).toBe(false);
  });

  it("nobody shares internet — no \"Other exits\" column", () => {
    const base = baseState({ domains: DOMAINS });
    mounted = mountSection(DomainsSection, { state: { ...base, peers: base.peers.map((p) => (p.self ? p : { ...p, exit: false })) } });
    expect(mounted.wrapper.text()).not.toContain("Other exits");
  });

  it("legend — a list: badge = verdict text, eight items", () => {
    const { wrapper } = mountDomains();
    const items = wrapper.get("#domains-legend").findAll("li");
    expect(items).toHaveLength(8);
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
