/* The "Site checks" tab on a node with two networks (A = `main` "friends", B "my devices"): one row per site across the
 * networks, member columns grouped by network, and every write going to each network that needs it. The whole page runs on
 * a fake node whose network routes record what they receive. */

import { afterEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { CHECK_ALL_CONFLICT } from "./domains-view.ts";

const SELF_ID = "aaaa1111";
const PEER_A1 = "bbbb2222";
const PEER_A2 = "cccc3333";
const PEER_B1 = "dddd4444";
const DOMAINS_A = "/api/domains";
const DOMAINS_B = "/api/net/" + NET_B + "/domains";
const CHECK_ALL_A = "/api/actions/check-all";
const CHECK_ALL_B = "/api/net/" + NET_B + "/actions/check-all";
const CHECK_DOMAIN_A = "/api/actions/check-domain";
const CHECK_DOMAIN_B = "/api/net/" + NET_B + "/actions/check-domain";
const FAILURE = { status: 500, body: { error: "internal" } };
const BUSY = { status: 409, body: { error: "busy" } };

const check = (verdict) => ({ verdict, reason: verdict, age_sec: 5 });

afterEach(unmountPage);

describe("DomainsSection with two networks", () => {
  describe("a site in both networks", () => {
    it("is one row, not one per network", async () => {
      const { wrapper } = await setup();
      const rows = wrapper.findAll("#domains-tbody tr");
      expect(rows, "one row for example.org").toHaveLength(1);
      expect(rows[0].text()).toContain("example.org");
    });

    it("has member columns grouped by network under a header with the network names", async () => {
      const { wrapper } = await setup();
      const heads = wrapper.findAll("thead th");
      const groupA = heads.find((th) => th.text().includes("friends"));
      const groupB = heads.find((th) => th.text().includes("my devices"));
      expect(groupA, "header of network A").toBeTruthy();
      expect(groupB, "header of network B").toBeTruthy();
      expect(groupA.attributes("colspan"), "two members in A").toBe("2");
      const at = (group) => heads.findIndex((th) => th.element === group.element);
      expect(at(groupA) < at(groupB), "A before B").toBe(true);
    });

    it("shows the marks of the members of A, then of B, after the \"For you\" mark", async () => {
      const { wrapper } = await setup();
      expect(cells(wrapper, "example.org").slice(0, 5)).toEqual(["example.org", "blocked by ISP", "opens", "site is down", "blocked as a bot"]);
    });

    it("takes \"For you\" from the first network where the site has a mark", async () => {
      const { wrapper } = await setup({ selfInA: null });
      expect(cells(wrapper, "example.org")[1]).toBe("opens");
    });
  });

  describe("a site only in A", () => {
    it("has a row, and its cells of network B are \"—\"", async () => {
      const { wrapper } = await setup({ onlyA: true });
      const row = cells(wrapper, "only-a.org");
      expect(row[0]).toBe("only-a.org");
      expect(row[2]).toBe("opens");
      expect(row[4], "member of B").toBe("—");
    });
  });

  describe("the footer", () => {
    it("says the list is stored only on this computer and members of the networks can't see it", async () => {
      const { wrapper } = await setup();
      expect(wrapper.get("#sec-domains").text()).toContain("The site list is stored only on this computer: members of your networks");
    });
  });

  describe("\"Add\"", () => {
    it("sends the site to both networks, the main one by the old path, and clears the field", async () => {
      const { wrapper, node } = await setup();
      await add(wrapper, "new.org");
      expect(posts(node, "POST", "/domains")).toEqual([DOMAINS_A, DOMAINS_B]);
      expect(node.calls.filter((c) => c.path.endsWith("/domains")).map((c) => c.body)).toEqual([{ domain: "new.org" }, { domain: "new.org" }]);
      expect(wrapper.get("#domain-add-input").element.value).toBe("");
      expect(wrapper.get("#domain-add-error").text()).toBe("");
    });

    it("a failure in B: the error names B, the site is in A and the field is cleared", async () => {
      const { wrapper, node } = await setup({ failB: true });
      await add(wrapper, "new.org");
      const error = wrapper.get("#domain-add-error").text();
      expect(error, "error with the name of B").toContain("\"my devices\"");
      expect(error.includes("\"friends\""), "A has no error").toBe(false);
      expect(node.slotList[0].state.domains.map((d) => d.name)).toContain("new.org");
      await polls(node, 1);
      expect(wrapper.get("#domains-tbody").text()).toContain("new.org");
    });

    it("repeated after a failure in B goes only to B", async () => {
      const { wrapper, node } = await setup({ failB: true });
      await add(wrapper, "new.org");
      await polls(node, 1);
      node.slotList[1].routes["POST /api/domains"] = acceptDomain;
      node.calls.length = 0;
      await add(wrapper, "new.org");
      expect(posts(node, "POST", "/domains")).toEqual([DOMAINS_B]);
      expect(wrapper.get("#domain-add-error").text()).toBe("");
    });

    it("an empty field sends nothing", async () => {
      const { wrapper, node } = await setup();
      await add(wrapper, "  ");
      expect(posts(node, "POST", "/domains")).toEqual([]);
    });
  });

  describe("\"Remove\"", () => {
    it("deletes the site from both networks", async () => {
      const { wrapper, node, urls } = await setup();
      await wrapper.get("button[aria-label=\"Remove example.org\"]").trigger("click");
      await settle();
      expect(posts(node, "DELETE", "/domains")).toEqual([DOMAINS_A, DOMAINS_B]);
      expect(urls.filter((u) => u.startsWith("DELETE"))).toEqual([
        "DELETE " + DOMAINS_A + "?name=example.org",
        "DELETE " + DOMAINS_B + "?name=example.org",
      ]);
    });

    it("deletes a site that is only in A from A alone", async () => {
      const { wrapper, node } = await setup({ onlyA: true });
      await wrapper.get("button[aria-label=\"Remove only-a.org\"]").trigger("click");
      await settle();
      expect(posts(node, "DELETE", "/domains")).toEqual([DOMAINS_A]);
    });
  });

  describe("\"Recheck\"", () => {
    it("asks every network that holds the site", async () => {
      const { wrapper, node } = await setup({ onlyA: true });
      await wrapper.get("button[aria-label=\"Recheck example.org\"]").trigger("click");
      await settle();
      await wrapper.get("button[aria-label=\"Recheck only-a.org\"]").trigger("click");
      await settle();
      expect(posts(node, "POST", "/actions/check-domain")).toEqual([CHECK_DOMAIN_A, CHECK_DOMAIN_B, CHECK_DOMAIN_A]);
    });
  });

  describe("\"Check all\"", () => {
    it("starts the check in both networks", async () => {
      const { wrapper, node } = await setup();
      await wrapper.get("#domains-check-all-btn").trigger("click");
      await settle();
      expect(posts(node, "POST", "/actions/check-all")).toEqual([CHECK_ALL_A, CHECK_ALL_B]);
    });

    it("a \"busy\" answer of B while A took it is not a conflict", async () => {
      const { wrapper, node } = await setup();
      node.slotList[1].routes["POST /api/actions/check-all"] = BUSY;
      await wrapper.get("#domains-check-all-btn").trigger("click");
      await settle();
      expect(wrapper.get("#domains-check-all-status").text()).toBe("");
    });

    it("\"busy\" from both networks is the conflict message", async () => {
      const { wrapper, node } = await setup();
      node.routes["POST /api/actions/check-all"] = BUSY;
      node.slotList[1].routes["POST /api/actions/check-all"] = BUSY;
      await wrapper.get("#domains-check-all-btn").trigger("click");
      await settle();
      expect(wrapper.get("#domains-check-all-status").text()).toBe(CHECK_ALL_CONFLICT);
    });
  });
});

describe("DomainsSection with one network", () => {
  it("\"Add\" sends one request to the old path, and there are no network headers", async () => {
    const node = createFakeNode({ state: demoState(), config: demoConfig(), routes: { "POST /api/domains": () => ({}) } });
    const { wrapper } = await mountPage(node);
    await openTab(wrapper, "domains");
    await add(wrapper, "new.org");
    expect(node.calls.filter((c) => c.method === "POST" && c.path.endsWith("/domains")).map((c) => c.path)).toEqual([DOMAINS_A]);
    expect(node.calls.some((c) => c.path.startsWith("/api/net/")), "no request to a network address").toBe(false);
    expect(wrapper.get("#domains-head-row").text()).not.toContain("friends");
    expect(wrapper.get("#sec-domains").text()).toContain("other members can't see or change it");
  });
});

// Opens the tab on a node with two networks. Options: `onlyA` adds a site that only A holds, `failB` makes B refuse a new
// site, `selfInA: null` leaves A without a mark for you. Returns the page, the node and the requests as "METHOD path?query".
async function setup({ onlyA = false, failB = false, selfInA = check("network_blocked") } = {}) {
  const node = createFakeNode(options({ onlyA, selfInA }));
  routes(node, failB);
  const urls = [];
  const rawFetch = node.fetch;
  node.fetch = (input, init = {}) => {
    const url = new URL(String(input), "http://node.test");
    urls.push((init.method || "GET").toUpperCase() + " " + url.pathname + url.search);
    return rawFetch(input, init);
  };
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "domains");
  node.urls = urls;
  return { wrapper, node, urls };
}

function options({ onlyA, selfInA }) {
  const opts = twoNets();
  const [a, b] = [opts.state, opts.slots[1].state];
  const self = b.peers.find((p) => p.self);
  b.peers = [self, { id: PEER_B1, id_full: "D".repeat(64), name: "phone", country: null, version: "0.1.0", online: true, path: "direct", rtt_ms: 20, exit: false }];
  a.domains = [{ name: "example.org", checks: { [SELF_ID]: selfInA, [PEER_A1]: check("ok"), [PEER_A2]: check("down") } }];
  b.domains = [{ name: "example.org", checks: { [SELF_ID]: check("ok"), [PEER_B1]: check("antibot") } }];
  if (onlyA) a.domains.push({ name: "only-a.org", checks: { [PEER_A1]: check("ok") } });
  // The shared fixture publishes a port without an audience, which the Ports tab (rendered, though hidden) can't draw.
  for (const config of [opts.config, opts.slots[1].config]) config.ports.published = [{ port: 8080, name: "web8080", for: "all" }];
  return { ...opts, state: a };
}

const acceptDomain = (body, slot) => {
  slot.state.domains.push({ name: body.domain, checks: {} });
  return {};
};

// Every route of both networks that the tab writes to; `failB` makes B refuse a new site.
function routes(node, failB) {
  const [a, b] = [node, node.slotList[1]];
  for (const slot of [a, b]) {
    slot.routes["POST /api/domains"] = slot === b && failB ? FAILURE : acceptDomain;
    slot.routes["DELETE /api/domains"] = () => ({});
    slot.routes["POST /api/actions/check-domain"] = () => ({});
    slot.routes["POST /api/actions/check-all"] = () => ({});
  }
}

async function add(wrapper, name) {
  await wrapper.get("#domain-add-input").setValue(name);
  await wrapper.get("#domain-add-btn").trigger("click");
  await settle();
}

// Paths of the node's requests with this method that end with `tail`, in the order they were sent.
const posts = (node, method, tail) => node.calls.filter((c) => c.method === method && c.path.endsWith(tail)).map((c) => c.path);

// Texts of the cells of a site's row.
function cells(wrapper, name) {
  const row = wrapper.findAll("#domains-tbody tr").find((tr) => tr.text().includes(name));
  expect(row, "row of " + name).toBeTruthy();
  return row.findAll("td").map((td) => td.text().replace(/\s+/g, " ").trim());
}
