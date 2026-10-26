/* "Diagnostics" and the join card with a network on a fake node (moved from e2e: network 42 and 59): the whole page,
 * the real `api.js` and the poller. Visibility is checked by `hidden`/`style`/`open`: happy-dom does not compute styles. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shownStrict as shown } from "../testing/dom.mjs";

const TYPED = "abc";

afterEach(unmountPage);

const makeNode = () => createFakeNode({ state: demoState(), config: demoConfig() });

// Expand "Details for specialists": a click on `<summary>` in happy-dom does not toggle `<details>`.
async function openSpecialist(wrapper) {
  const details = wrapper.get("#diag-specialist");
  if (details.element.open) return;
  details.element.open = true;
  await details.trigger("toggle");
  await settle();
}

const headingsNamed = (wrapper, name) => wrapper.findAll("h1,h2,h3,h4,h5,h6").filter((h) => h.text().trim() === name);

describe("texts of the join and key cards when a network exists", () => {
  it("Settings offers \"Join or create another network\" instead of a switch, the \"Mesh Town network key\" card with a \"Show\" button", async () => {
    const { wrapper } = await mountPage(makeNode());
    await openTab(wrapper, "settings");

    expect(headingsNamed(wrapper, "Switch to another network")).toHaveLength(0);
    expect(wrapper.find("#join-card").exists()).toBe(false);
    expect(wrapper.get("#net-join-slot a").text()).toBe("Join or create another network");

    await openTab(wrapper, "network");
    await openSpecialist(wrapper);
    const card = wrapper.get('[aria-labelledby="net-key-title"]');
    expect(card.get("h2").text()).toBe("Mesh Town network key");
    expect(shown(card.element)).toBe(true);
    const show = card.findAll("button").filter((b) => b.text().trim() === "Show");
    expect(show).toHaveLength(1);
    expect(shown(show[0].element)).toBe(true);
  });
});

describe("\"Diagnostics\" is not rebuilt by a poll", () => {
  it("the \"The member's addresses\" field and \"Details for specialists\" survive two polls — same nodes, value and focus intact, details open, \"Checks\" not empty", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);
    await openTab(wrapper, "network");
    await openSpecialist(wrapper);
    const field = wrapper.get("#peer-addr-input");
    const summary = wrapper.get("#diag-specialist summary");
    field.element.focus();
    await field.setValue(TYPED);

    await polls(node, 2);

    expect(field.element.isConnected, "the field node was recreated by a poll").toBe(true);
    expect(summary.element.isConnected, "the summary node was recreated by a poll").toBe(true);
    expect(wrapper.get("#peer-addr-input").element).toBe(field.element);
    expect(wrapper.get("#diag-specialist summary").element).toBe(summary.element);
    expect(field.element.value).toBe(TYPED);
    expect(document.activeElement).toBe(field.element);
    expect(shown(wrapper.get('[aria-labelledby="net-key-title"]').element)).toBe(true);
    expect(wrapper.findAll('[aria-label="Checks"] li').length, "rows in \"Checks\"").toBeGreaterThanOrEqual(3);
  });
});
