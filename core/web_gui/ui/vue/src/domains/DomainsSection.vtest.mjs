import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { PEER_FULL, baseState, mountSection } from "../testing/section.mjs";
import DomainsSection from "./DomainsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const DOMAINS = [
  { name: "example.org", checks: { aaaa1111: { verdict: "network_blocked" }, bbbb2222: { verdict: "ok" } } },
  { name: "fresh.test", checks: {} },
];
let mounted;
const mountDomains = (domains = DOMAINS) => (mounted = mountSection(DomainsSection, { state: baseState({ domains }) }));

// peer-one is the active exit and opens example.org, three more exits open it too: "also via 3 more".
const extraPeer = (n) => ({ id: n.repeat(8), id_full: n.toUpperCase().repeat(64), name: "extra-" + n, country: null, version: null, online: true, path: "direct", rtt_ms: 30, exit: true });
const manyState = () => {
  const extra = ["d", "e", "f"].map(extraPeer);
  const checks = Object.fromEntries([["aaaa1111", { verdict: "network_blocked" }], ["bbbb2222", { verdict: "ok" }], ...extra.map((p) => [p.id, { verdict: "ok" }])]);
  const base = baseState({ domains: [{ name: "example.org", checks }], route: { exits: [{ id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, exit: true, country: null, active: true }] } });
  return { ...base, peers: [...base.peers, ...extra] };
};

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
    expect(rows[0].text()).toContain("blocked by provider");
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

  it("\"3 more\" opens and closes the same member list as Details", async () => {
    mounted = mountSection(DomainsSection, { state: manyState() });
    const { wrapper } = mounted;
    const more = wrapper.findAll("tbody button").find((b) => b.text() === "3 more");
    const details = wrapper.findAll("tbody button").find((b) => b.text() === "Details");
    expect(wrapper.get(".domain-others").text()).toContain("also via 3 more");
    expect(more.attributes("aria-expanded")).toBe("false");
    await more.trigger("click");
    expect(wrapper.find(".site-members").exists()).toBe(true);
    expect(more.attributes("aria-expanded")).toBe("true");
    expect(details.attributes("aria-expanded")).toBe("true");
    expect(details.text()).toBe("Hide");
    await more.trigger("click");
    expect(wrapper.find(".site-members").exists()).toBe(false);
    expect(details.text()).toBe("Details");
  });

  it("no counter — no button in the \"Other exits\" cell", () => {
    const { wrapper } = mountDomains();
    expect(wrapper.get(".domain-others").text()).toContain("opens via peer-one");
    expect(wrapper.find(".domain-others .site-more").exists()).toBe(false);
  });

  it("recheck and remove glyphs are svg inside the buttons, labels unchanged", () => {
    const { wrapper } = mountDomains();
    const recheck = wrapper.get('tbody button[aria-label="Recheck example.org"]');
    const remove = wrapper.get('tbody button[aria-label="Remove example.org"]');
    expect(recheck.find("svg").exists()).toBe(true);
    expect(remove.find("svg").exists()).toBe(true);
    expect(recheck.text()).toBe("");
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

  it("legend — closed details below the list; no jargon anywhere in the tab", () => {
    const { wrapper } = mountDomains();
    const legend = wrapper.get("details#domains-legend");
    expect(legend.attributes("open")).toBeUndefined();
    expect(legend.get("summary").text()).toBe("What the marks mean");
    expect(legend.findAll(".badge")).toHaveLength(8);
    const text = wrapper.text();
    for (const word of ["ISP", "spoofed", "bot", "column"]) expect(text).not.toMatch(new RegExp("\\b" + word + "\\b"));
  });

  it("order: form, hint, no-exit note, status, list, legend", () => {
    const base = baseState({ domains: DOMAINS });
    mounted = mountSection(DomainsSection, { state: { ...base, peers: base.peers.map((p) => (p.self ? p : { ...p, exit: false })) } });
    const html = mounted.wrapper.html();
    const at = (s) => html.indexOf(s);
    const order = ["domain-add-btn", 'id="domains-hint"', 'id="domains-no-exit"', 'id="domains-check-all-status"', 'id="domains-exit-status"', 'id="domains-tbody"', 'id="domains-legend"'].map(at);
    expect(order.every((n) => n >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });

  it("wide: an action error is its own row under the site, not in the actions cell", async () => {
    api.mockRejectedValue(new ApiError(500, "boom", "Server said no"));
    const { wrapper } = mountDomains();
    await wrapper.findAll("tbody button").find((b) => /^Recheck /.test(b.attributes("aria-label") || "")).trigger("click");
    await flushPromises();
    const row = wrapper.get("tr.domain-error");
    expect(row.get("td").attributes("colspan")).toBeTruthy();
    expect(row.text()).toContain("Couldn't recheck example.org");
    expect(wrapper.find(".domain-actions .field-error-text").exists()).toBe(false);
  });

  it("legend — a list: badge = verdict text, eight items", () => {
    const { wrapper } = mountDomains();
    const items = wrapper.get("#domains-legend").findAll("li");
    expect(items).toHaveLength(8);
    expect(items[0].get(".badge").text()).toBe("opens");
    expect(items[2].text()).toContain("your internet provider doesn't let the connection through");
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
