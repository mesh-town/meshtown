/* An end-to-end case of the Shared services panel without a binary and a browser (moved from e2e: ports 56): the whole page on
 * a fake node. Visibility is judged by `hidden`/`style`: happy-dom does not compute styles. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const LOCAL_PORT = "39123";

afterEach(unmountPage);

describe("ports element survives poll", () => {
  it("the \"Network ports\" row survives two polls — the same element, the entered number in place, \"Open\" works", async () => {
    // The demo node shows the network port as closed while `ports.opened` is empty (in `demoState` it is already "opened").
    const state = demoState();
    state.ports.network[0].opened = null;
    const node = createFakeNode({ state, config: demoConfig() });
    node.routes["PUT /api/config"] = (body) => nodeAppliesPut(node, body);
    const { wrapper } = await mountPage(node, { hash: "peers/main/services" });
    const row = wrapper.get("#ports-net-tbody tr");
    expect(shown(row.element)).toBe(true);
    const input = row.get("input");
    await input.setValue(LOCAL_PORT);

    await polls(node, 2);
    expect(row.element.isConnected).toBe(true);
    expect(input.element.isConnected).toBe(true);
    expect(wrapper.get("#ports-net-tbody tr input").element).toBe(input.element);
    expect(input.element.value).toBe(LOCAL_PORT);

    await row.findAll("button").find((b) => b.text() === "Open on this computer").trigger("click");
    await settle();
    // The port in "opened" is taken from the PUT body by the fake node (like a real one), not by the test: the page sent the number itself.
    expect(node.config.ports.opened.map((o) => o.local)).toEqual([Number(LOCAL_PORT)]);
    expect(node.state.ports.network[0].opened.local).toBe(Number(LOCAL_PORT));
    await polls(node, 1);
    expect(wrapper.get("#ports-net-tbody tr").text()).toContain("127.0.0.1:" + LOCAL_PORT);
  });
});

// Accepts a PUT as a node: writes the config and sets "opened" in the snapshot from the first port in the request body.
function nodeAppliesPut(node, body) {
  node.config = structuredClone(body.config);
  node.rev += 1;
  const [first] = node.config.ports.opened;
  node.state.ports.network[0].opened = { local: first.local, state: "listening", error: null };
  return { status: 200, body: { rev: node.rev, config: node.config, changed: [], file: null, read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] } };
}
