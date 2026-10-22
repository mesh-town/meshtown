/* End-to-end case for the "Settings" tab without the binary and the browser (moved from e2e: save 58): the whole page on
 * a mock node. The "unsaved" label on the tab is the `has-dirty` class (the `class` attribute, not the computed style). */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const PORT_LABEL = "Proxy port (SOCKS5 and HTTP)";
const TYPED = "1081";

afterEach(unmountPage);

const portField = (wrapper) => wrapper.get(`input[aria-label="${PORT_LABEL}"]`);
const resetButton = (wrapper) => wrapper.findAll("#sec-settings button").find((b) => b.text() === "Discard changes");
const tabDirty = (wrapper) => wrapper.get("#tab-settings").classes().includes("has-dirty");

describe("the settings element survives polling", () => {
  it("the \"Proxy port\" field with an edit and the \"Discard changes\" button survive two polls — same nodes, value and focus intact, label in place; the click works the first time", async () => {
    const node = createFakeNode({ state: demoState(), config: demoConfig() });
    const { wrapper } = await mountPage(node);
    await openTab(wrapper, "settings");
    const field = portField(wrapper);
    expect(shown(field.element)).toBe(true);
    const configValue = field.element.value;
    expect(configValue).not.toBe(TYPED);

    field.element.focus();
    await field.setValue(TYPED);
    await settle();
    expect(tabDirty(wrapper)).toBe(true);
    const reset = resetButton(wrapper);

    await polls(node, 2);
    expect(field.element.isConnected).toBe(true);
    expect(reset.element.isConnected).toBe(true);
    expect(portField(wrapper).element).toBe(field.element);
    expect(resetButton(wrapper).element).toBe(reset.element);
    expect(field.element.value).toBe(TYPED);
    expect(document.activeElement).toBe(field.element);
    expect(tabDirty(wrapper)).toBe(true);

    await reset.trigger("click");
    await settle();
    expect(portField(wrapper).element.value).toBe(configValue);
    expect(tabDirty(wrapper)).toBe(false);
  });
});
