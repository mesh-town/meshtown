/* End-to-end cases of the "Site checks" tab with no binary and no browser (moved from e2e: domains-hint, domains-you,
 * domains): the whole page on a stub node. Visibility is judged by `hidden`/`style`: happy-dom doesn't compute styles. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState, DEMO_PEER_ONE } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const SELF_ID = "aaaa1111";
const GOOD = { verdict: "ok", reason: "code 200", age_sec: 5 };
const BAD = { verdict: "network_blocked", reason: "TCP: reset", age_sec: 5 };

afterEach(unmountPage);

const makeNode = (state = demoState(), routes = {}) => createFakeNode({ state, config: demoConfig(), routes });

async function openDomains(node) {
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "domains");
  return wrapper;
}

const recheckButton = (wrapper) => wrapper.findAll("table button").find((b) => /^Recheck /.test(b.attributes("aria-label") || ""));
const firstRow = (wrapper) => wrapper.get("#domains-tbody tr");
const checkAllButton = (wrapper) => wrapper.get("#domains-check-all-btn");

// State: the site is blocked for you, it opens at peer-one (an exit, online).
function openViaState() {
  const state = demoState();
  state.domains[0].checks = { [SELF_ID]: BAD, [DEMO_PEER_ONE.id]: GOOD };
  return state;
}

describe("domains hint own list", () => {
  it("the tab hints don't call the list shared — it is stored only on this computer", async () => {
    const wrapper = await openDomains(makeNode());
    const text = wrapper.get("#sec-domains").text().trim();
    expect(text.length).toBeGreaterThan(0);
    expect(text.includes("shared")).toBe(false);
    expect(text.includes("only on this computer")).toBe(true);
  });
});

describe("domains you column legend", () => {
  // The node answers "Check all": it records its own check in every row (like the demo node).
  const checkAllRoute = (state) => () => {
    for (const d of state.domains) d.checks[SELF_ID] = GOOD;
    return {};
  };

  it("header \"Site\", \"For you\"; the legend is visible; after \"Check all\" the \"For you\" column is not empty", async () => {
    const state = demoState();
    const node = makeNode(state, { "POST /api/actions/check-all": checkAllRoute(state) });
    const wrapper = await openDomains(node);
    const heads = wrapper.findAll("#domains-head-row th").map((h) => h.text());
    expect(heads[0]).toBe("Site");
    expect(heads[1]).toBe("For you");
    const peerHead = wrapper.findAll("#domains-head-row th")[2];
    expect(peerHead.find("br").exists()).toBe(true);
    expect(peerHead.get(".peer-code").text()).toBe(peerHead.attributes("title"));

    const legend = wrapper.get("#domains-legend");
    expect(shown(legend.element)).toBe(true);
    expect(legend.findAll("li")).toHaveLength(7);

    expect(recheckButton(wrapper)).toBeTruthy();
    expect(firstRow(wrapper).findAll("td")[1].text()).toBe("—");
    await checkAllButton(wrapper).trigger("click");
    await polls(node, 1);
    expect(node.count("POST", "/api/actions/check-all")).toBe(1);
    expect(checkAllButton(wrapper).text()).toBe("Check all");
    expect(firstRow(wrapper).findAll("td")[1].text()).not.toBe("—");
  });
});

describe("domains open via peer", () => {
  it("blocked for you, opens for a member — the \"Exit via …\" button makes them the exit", async () => {
    const node = makeNode(openViaState());
    const wrapper = await openDomains(node);
    const button = firstRow(wrapper).findAll("button").find((b) => /^Exit via /.test(b.text()));
    expect(shown(button.element)).toBe(true);

    await button.trigger("click");
    await settle();
    expect(firstRow(wrapper).text()).toContain("✓ Your exit:");
    expect(node.config.route.exits).toEqual([DEMO_PEER_ONE.id_full]);
  });
});

describe("domains element survives poll", () => {
  it("the table row survives two polls — the same node, not a recreated one", async () => {
    const node = makeNode();
    const wrapper = await openDomains(node);
    const before = recheckButton(wrapper);
    expect(shown(before.element)).toBe(true);

    await polls(node, 2);
    expect(before.element.isConnected).toBe(true);
    expect(recheckButton(wrapper).element).toBe(before.element);
    expect(recheckButton(wrapper).attributes("aria-label")).toMatch(/^Recheck \S+/);
  });
});
