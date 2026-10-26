/* The banner after creating a network goes for good once somebody else is in that network; a member of another network changes nothing. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { resetNetBanner, useNetBanner } from "../home/add-net-banner.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, unmountPage } from "../testing/page.mjs";
import { PEER_FULL, baseState } from "../testing/section.mjs";

const SELF = { self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "finch-unwary" };
const PEER = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, path: "direct", rtt_ms: 30 };
const stateWith = (peers) => baseState({ network: { id_hex: "net-bbbb", masked: "mesh2-••••-••••" }, peers });

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

async function open() {
  const node = createFakeNode(twoNets({ state: stateWith([SELF]) }));
  useNetBanner().show(NET_B, "You created it.");
  const { wrapper } = await mountPage(node, { hash: "peers/" + NET_B });
  return { node, wrapper };
}

const bannerOf = () => useNetBanner().banner.value;

describe("the banner of a created network", () => {
  it("stays while another network gets a member", async () => {
    const { node, wrapper } = await open();
    expect(bannerOf()?.slot).toBe(NET_B);
    expect(wrapper.find("#peers-net-banner").exists()).toBe(true);
    await polls(node, 1);
    expect(bannerOf()?.slot).toBe(NET_B);
  });

  it("the banner goes once someone joins", async () => {
    const { node, wrapper } = await open();
    node.slotList.find((s) => s.id === NET_B).state = stateWith([SELF, PEER]);
    await polls(node, 1);
    expect(bannerOf()).toBeNull();
    expect(wrapper.find("#peers-net-banner").exists()).toBe(false);
  });

  it("does not come back when the member leaves again", async () => {
    const { node } = await open();
    const slot = node.slotList.find((s) => s.id === NET_B);
    slot.state = stateWith([SELF, PEER]);
    await polls(node, 1);
    slot.state = stateWith([SELF]);
    await polls(node, 1);
    expect(bannerOf()).toBeNull();
  });
});
