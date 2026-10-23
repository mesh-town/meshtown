/* The join card on a stub node (moved from e2e: home-proxy-join 30, network 16 and 42 "no network"):
 * the whole page, the real `api.js` and poller. Visibility is checked by the `hidden` attribute: happy-dom doesn't compute styles. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const JOIN_FALLBACK_TEXT = "This doesn't look like an invite. Copy it from the network member again in full and paste it here.";

afterEach(unmountPage);

const makeNode = (over = {}) => createFakeNode({ state: demoState(), config: demoConfig(), ...over });
const noNetState = () => demoState({ network: null, peers: [], domains: [], ports: { published: [], network: [] } });

async function submitKey(wrapper, key) {
  await wrapper.get("#join-key-input").setValue(key);
  await wrapper.get("#join-btn").trigger("click");
  await settle();
}

describe("a join error without message", () => {
  it("the node replied 422 without message — #join-key-error shows the page's verbatim fallback text", async () => {
    const node = makeNode({ state: noNetState(), routes: { "POST /api/network/join": { status: 422, body: { error: "invalid_key" } } } });
    const { wrapper } = await mountPage(node);
    await wrapper.get("#home-start-join-btn").trigger("click");
    await settle();
    await submitKey(wrapper, "webcheck-not-an-invite");

    const error = wrapper.get("#join-key-error");
    expect(shown(error.element)).toBe(true);
    expect(error.text()).toBe(JOIN_FALLBACK_TEXT);
  });
});

describe("network card texts (no network): the join card", () => {
  it("network card texts (no network): the title and button are \"Join with an invite\" and \"Join\", one each", async () => {
    const { wrapper } = await mountPage(makeNode({ state: noNetState() }));
    await wrapper.get("#home-start-join-btn").trigger("click");
    await settle();

    const headings = wrapper.findAll("h1,h2,h3,h4,h5,h6").filter((h) => h.text().trim() === "Join with an invite");
    const buttons = wrapper.findAll("button").filter((b) => b.text().trim() === "Join");
    expect(headings).toHaveLength(1);
    expect(buttons).toHaveLength(1);
  });
});
