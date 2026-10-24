/* A join that the members refused, on the Members page: the form leads to the tab of that slot, which keeps the refusal
 * text and "Back" until the user goes back. Same fake node as `PeersSection.add-net.vtest.mjs`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetNetBanner } from "../home/add-net-banner.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig } from "../testing/section.mjs";

const FRESH = "neta22";
const MESSAGE = "The members did not accept this invite.";

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

describe("the members refused the join", () => {
  it("the page moves from the form to the tab of that network with the text and Back", async () => {
    const page = await waitingPage();
    expect(location.hash).toBe("#peers/add");
    await page.refuse();
    expect(location.hash).toBe("#peers/" + FRESH);
    expect(page.tab().text()).toContain("refused");
    expect(page.wrapper.get("#sec-peers #net-join-refused").text()).toBe(MESSAGE);
    expect(page.wrapper.get("#sec-peers #net-join-cancel").text()).toBe("Back");
  });

  it("the tab stays across polls until Back is pressed, then it is gone", async () => {
    const page = await waitingPage();
    await page.refuse();
    await polls(page.node, 2);
    expect(page.tab().exists()).toBe(true);
    await page.wrapper.get("#net-join-cancel").trigger("click");
    await vi.advanceTimersByTimeAsync(0);
    await settle();
    expect(page.tab().exists()).toBe(false);
  });
});

async function waitingPage() {
  const opts = { ...twoNets(), config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) };
  opts.slots.push({ id: FRESH });
  const node = createFakeNode(opts);
  const slot = node.slotList.find((s) => s.id === FRESH);
  slot.joining = true;
  const { wrapper } = await mountPage(node, { hash: "peers/add" });
  await polls(node, 1);
  const tab = () => wrapper.findAll("#sec-peers .ntabs a").find((t) => t.attributes("href") === "#peers/" + FRESH) ?? { exists: () => false };
  const refuse = async () => {
    slot.joining = false;
    slot.state = { ...slot.state, node: { ...slot.state.node, invite_join: { state: "refused", message: MESSAGE } } };
    await polls(node, 1);
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  return { node, wrapper, tab, refuse };
}
