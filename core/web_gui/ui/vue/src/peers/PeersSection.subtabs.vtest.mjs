/* The sub-tabs inside a network tab of "Networks", the whole page on a fake node: Members, Shared services (ports), Device settings.
 * Contract: the row is `#net-subtabs` (`#net-sub-<sub>`), the panel `#net-subpanel`; the pane is chosen by `store.netSub`.
 * No sub-tabs without a network, on an excluded computer, during a join and on the "+" tab. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const FRESH = "neta22";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

describe("each pane by store.netSub", () => {
  it("members: the member list, no ports form, no device block", async () => {
    const page = await openTwo("peers/main");
    page.expectOnlyPane("#peers-list");
    expect(page.selectedSub()).toBe("net-sub-members");
  });

  it("services: the ports of this network, no member list", async () => {
    const page = await openTwo("peers/main/services");
    page.expectOnlyPane("#port-add-btn");
    expect(page.selectedSub()).toBe("net-sub-services");
  });

  it("device: the block of this computer in the network, no member list", async () => {
    const page = await openTwo("peers/main/device");
    page.expectOnlyPane("#you-block");
    expect(page.selectedSub()).toBe("net-sub-device");
  });

  it("the store switches the pane without a new address", async () => {
    const page = await openTwo("peers/main");
    page.store.netSub = "services";
    await settle();
    page.expectOnlyPane("#port-add-btn");
    page.store.netSub = "device";
    await settle();
    page.expectOnlyPane("#you-block");
  });

  it("the panel is labelled by the active sub-tab", async () => {
    const page = await openTwo("peers/main/services");
    expect(page.wrapper.get("#net-subpanel").attributes("role")).toBe("tabpanel");
    expect(page.wrapper.get("#net-subpanel").attributes("aria-labelledby")).toBe("net-sub-services");
  });

  it("the head of the network stays above the sub-tabs on every pane", async () => {
    const page = await openTwo("peers/" + NET_B + "/services");
    expect(page.wrapper.find("#peers-tabhead").exists()).toBe(true);
  });
});

describe("no network", () => {
  it("no network — no subtabs", async () => {
    const page = await open(noNetworkNode(), "peers");
    expect(page.wrapper.find("[data-no-network-msg]").exists()).toBe(true);
    expect(page.has("#net-subtabs")).toBe(false);
    expect(page.has("#net-subpanel")).toBe(false);
  });
});

describe("one network", () => {
  it("one network — three subtabs", async () => {
    const page = await open(oneNode(), "peers");
    expect(page.subLabels()).toEqual(["Members", "Shared services (ports)", "Device settings"]);
  });

  it("one network: Leave and Delete in Device settings", async () => {
    const page = await open(oneNode(), "peers/main/device");
    expect(page.has("#you-leave"), "Leave").toBe(true);
    expect(page.has("#you-block")).toBe(true);
  });
});

describe("two and eight networks", () => {
  it("two networks — three subtabs under the head", async () => {
    const page = await openTwo("peers/" + NET_B);
    expect(page.subLabels()).toEqual(["Members", "Shared services (ports)", "Device settings"]);
  });

  it("eight — subtabs and full +", async () => {
    const page = await open(eightNode(), "peers/netb22");
    expect(page.subLabels()).toHaveLength(3);
    const tabs = page.wrapper.findAll("#sec-peers .ntabs a");
    expect(tabs).toHaveLength(9);
    expect(tabs.at(-1).attributes("aria-label")).toBe("Mesh Town can be in up to 8 networks at once");
  });
});

describe("excluded and joining", () => {
  it("excluded / joining — card, no subtabs: excluded", async () => {
    const opts = twoNets();
    opts.slots[1].members = membersReply({ me_excluded: true });
    const page = await open(createFakeNode(audienced(opts)), "peers/" + NET_B);
    expect(page.has("#members-removed-card")).toBe(true);
    expect(page.has("#net-subtabs")).toBe(false);
  });

  it("excluded / joining — card, no subtabs: joining", async () => {
    const opts = twoNets();
    opts.slots.push({ id: FRESH });
    const node = createFakeNode(opts);
    node.slotList.find((s) => s.id === FRESH).joining = true;
    const page = await open(node, "peers/" + FRESH);
    expect(page.has("#net-subtabs")).toBe(false);
    expect(page.has("#net-join-cancel"), "the waiting card").toBe(true);
  });
});

const audienced = (opts) => ({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });
const withPatch = (patch) => audienced(twoNets({ config: audienced({}).config, ...patch }));
const openTwo = (hash) => open(createFakeNode(withPatch({})), hash);
const oneNode = () => { const o = withPatch({}); return createFakeNode({ ...o, slots: [o.slots[0]] }); };
const noNetworkNode = () => createFakeNode({ state: baseState({ network: null, peers: [] }), config: baseConfig() });

function eightNode() {
  const o = withPatch({});
  const more = ["c", "d", "e", "f", "g", "h"].map((c) => ({ id: "net" + c + "22", label: "net " + c, id_hex: "net-" + c, members: membersReply(), state: baseState({ network: { id_hex: "net-" + c, masked: "mesh2-••••-••••" } }), config: o.config }));
  return createFakeNode({ ...o, slots: [...o.slots.slice(0, 1), { ...o.slots[1], id: "netb22" }, ...more].slice(0, 8) });
}

async function open(node, hash) {
  const { wrapper, store } = await mountPage(node, { hash });
  const has = (selector) => wrapper.find(selector).exists();
  const panes = ["#peers-list", "#port-add-btn", "#you-block"];
  return {
    wrapper, store, has,
    subLabels: () => wrapper.findAll("#net-subtabs [role=tab]").map((t) => t.text()),
    selectedSub: () => wrapper.findAll("#net-subtabs [role=tab]").find((t) => t.attributes("aria-selected") === "true")?.attributes("id"),
    expectOnlyPane(selector) {
      for (const p of panes) expect(has(p), p + (p === selector ? " is shown" : " is hidden")).toBe(p === selector);
    },
  };
}
