/* A slot whose saved settings cannot be read: the node lists it as broken with no state. The Members page shows it as a tab
 * ("can't start") and, on that tab, says why instead of "Loading…"; the live tab next to it is untouched. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

const REASON = "This network can't start: its saved settings can't be read, for example a network key from an older version. Ask an admin for a new invite to join it again.";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

// B is listed as the node lists a slot it could not start: no key, no label, no state.
function breakB(node) {
  const plain = node.fetch;
  node.fetch = async (input, init) => {
    const res = await plain(input, init);
    if (!String(input).startsWith("/api/overview")) return res;
    const raw = await res.json();
    Object.assign(raw.nets.find((n) => n.id === NET_B), { label: null, id_hex: null, status: "broken", peers_online: 0, peers_total: 0 });
    Object.assign(raw.slots.find((s) => s.slot === NET_B), { state: null, members: null });
    return new Response(JSON.stringify(raw), { status: 200, headers: { "content-type": "application/json" } });
  };
  return node;
}

async function open(hash) {
  const { wrapper } = await mountPage(breakB(createFakeNode(twoNets())), { hash });
  const tabs = () => wrapper.findAll("#sec-peers .ntabs a");
  return { wrapper, tabs, tabB: () => tabs().find((t) => t.attributes("href") === "#peers/" + NET_B) };
}

describe("a broken slot", () => {
  it('shows the broken tab with "can\'t start"', async () => {
    const page = await open("peers");
    expect(page.tabB().exists()).toBe(true);
    expect(page.tabB().text()).toContain("· can't start");
    expect(page.tabB().attributes("title")).toContain("can't start: its saved settings can't be read");
  });

  it("the broken tab shows the reason, not Loading…", async () => {
    const page = await open("peers/" + NET_B);
    expect(page.wrapper.get("#peers-broken-net").text()).toBe(REASON);
    expect(page.wrapper.get("#sec-peers").text()).not.toContain("Loading…");
  });

  it("a live tab next to it shows its network", async () => {
    const page = await open("peers/" + NET_A);
    expect(page.wrapper.find("#peers-broken-net").exists()).toBe(false);
    expect(page.wrapper.find("#peers-list").exists()).toBe(true);
  });
});
