/* End-to-end cases of the "Members" tab without a binary and a browser (moved from e2e: peer-exit 45 and 53): the whole page
 * on a fake node. Visibility is checked by `hidden`/`style`: happy-dom does not compute styles. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const EXIT_VIA = "Use as exit";
const EXIT_CHOSEN = ["✓ Your exit", "Your exit — unavailable right now"];
const REMEMBERED_KEY = "meshtown.exitsRemembered";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

const makeNode = () => createFakeNode({ state: demoState(), config: demoConfig() });

async function openPeers(node) {
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "peers");
  return wrapper;
}

const items = (wrapper) => wrapper.findAll("#peers-list > li");
const exitButtonRow = (wrapper) => items(wrapper).find((li) => li.find("button").exists());

describe("peers table headers", () => {
  it("\"Members\" — a <ul id=peers-list> list without a table, \"You\" as the first row", async () => {
    const node = makeNode();
    const wrapper = await openPeers(node);
    const list = wrapper.get("#peers-list");
    expect(list.element.tagName).toBe("UL");
    expect(shown(list.element)).toBe(true);
    expect(wrapper.get("#sec-peers").findAll("table")).toHaveLength(0);
    const first = items(wrapper)[0];
    expect(first.text()).toContain("You: ");
    expect(first.findAll("a")).toHaveLength(1);

    const self = node.state.peers.find((p) => p.self);
    expect(first.attributes("title")).toBe(self.id_full);
  });
});

describe("peers element survives poll", () => {
  it("the action button in a row survives two polls — the same node, the click works the first time", async () => {
    const node = makeNode();
    const wrapper = await openPeers(node);
    expect(shown(items(wrapper)[0].element)).toBe(true);
    const row = exitButtonRow(wrapper);
    const button = row.get("button");
    const code = row.get(".peer-code").text();
    const idFull = button.attributes("data-exit-via");
    const peerName = row.get(".peer-name").text().trim();
    expect(button.attributes("aria-label")).toBe(EXIT_VIA + ": " + peerName);

    await polls(node, 2);
    expect(button.element.isConnected).toBe(true);
    await button.trigger("click");
    await settle();
    const rowNow = items(wrapper).find((li) => li.text().includes(code));
    expect(EXIT_CHOSEN.some((t) => rowNow.text().includes(t))).toBe(true);
    expect(wrapper.get("#peers-exit-status").text()).toBe("Saved");
    expect(node.config.route.exits).toEqual([idFull]);
    expect(JSON.parse(localStorage.getItem(REMEMBERED_KEY))).toEqual([idFull]);
  });
});
