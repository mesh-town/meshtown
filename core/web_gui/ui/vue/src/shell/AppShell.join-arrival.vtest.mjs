/* A join from the "+" tab that waits for a member: when the slot arrives (or is refused) the page leads to
 * its tab if the user is still where he sent the form from; a user who went elsewhere stays there. The whole page, a fake node. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetNetBanner, useNetBanner } from "../home/add-net-banner.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const FRESH = "neta22";
const KEY = "mesh2-ABCDEFGH";
const MESSAGE = "The members did not accept this invite.";

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

describe("The "+" tab: a join that waits for a member", () => {
  it("a waiting join leads to the new tab and the card starts over", async () => {
    const page = await sentFromAddTab();
    expect(page.wrapper.find("#add-net-waiting").exists(), "the waiting card is shown first").toBe(true);
    page.arrives();
    await polls(page.node, 1);
    await page.hashChanged();
    expect(location.hash).toBe("#peers/" + FRESH);
    await page.goTo("peers/add");
    expect(page.wrapper.find("#add-net-waiting").exists()).toBe(false);
    expect(page.wrapper.get("#sec-peers #add-net-input").element.value).toBe("");
  });

  it("the user who left the "+" tab stays where he is, the banner waits on the tab", async () => {
    const page = await sentFromAddTab();
    await openTab(page.wrapper, "home");
    page.arrives();
    await polls(page.node, 1);
    await page.hashChanged();
    expect(location.hash).toBe("#home");
    expect(useNetBanner().banner.value?.slot).toBe(FRESH);
  });

  it("a refused join leads to the card of that slot with the refusal and \"Back\"", async () => {
    const page = await sentFromAddTab();
    page.refuses();
    await polls(page.node, 1);
    await page.hashChanged();
    expect(location.hash).toBe("#peers/" + FRESH);
    expect(page.wrapper.get("#sec-peers #net-join-refused").text()).toBe(`Couldn't join. ${MESSAGE}`);
    expect(page.wrapper.get("#sec-peers #net-join-cancel").text()).toBe("Back");
  });
});

function becomesNetwork(slot) {
  slot.joining = false;
  slot.label = "work";
  slot.id_hex = "net-cccc";
  slot.state = baseState({ network: { id_hex: "net-cccc", masked: "mesh2-••••-••••" }, peers: [] });
  slot.routes["GET /api/members"] = membersReply({ rows: [selfRow()] });
}

function refuse(slot) {
  slot.joining = false;
  slot.state = { ...slot.state, node: { ...slot.state.node, invite_join: { state: "refused", message: MESSAGE } } };
}

// The page on the "+" tab with a word key sent to a new slot that answers "waiting".
async function sentFromAddTab() {
  const opts = { ...twoNets(), config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) };
  opts.slots.push({ id: FRESH });
  const node = createFakeNode(opts);
  const slot = node.slotList.find((s) => s.id === FRESH);
  slot.routes["POST /api/network/join"] = () => { slot.joining = true; return { label: null, restart: false, waiting: true }; };
  const { wrapper } = await mountPage(node, { hash: "peers/add" });
  const hashChanged = async () => {
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  await wrapper.get("#sec-peers #add-net-input").setValue(KEY);
  await wrapper.get("#sec-peers #add-net-btn").trigger("click");
  await vi.advanceTimersByTimeAsync(0);
  await settle();
  await polls(node, 1);
  return {
    node, wrapper, hashChanged,
    arrives: () => becomesNetwork(slot),
    refuses: () => refuse(slot),
    async goTo(hash) {
      location.hash = hash;
      await hashChanged();
    },
  };
}
