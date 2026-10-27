/* "Home" on a stub node (moved from e2e: home-fields 23, 24; home 20, 26): the whole page, the real
 * `api.js`, poller and store; the config PUT is checked by the request body in `node.calls`. Visibility is judged by the
 * `hidden`/`style` attribute: happy-dom doesn't compute styles. The `#status-line` tail is not asserted (it stays in e2e). */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { PAC_INTRO, PAC_TITLE } from "../common/pac-texts.ts";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const STRANGER_ID = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
const STRANGER_LABEL = STRANGER_ID.slice(0, 10);
const LISTEN = "127.0.0.1:1080";
const EMPTY_KEY_HINT = "Paste the invite first.";

beforeEach(() => localStorage.setItem("meshtown.proxyScope", "system")); // the scope question is answered
afterEach(unmountPage);

const makeNode = (over = {}) => createFakeNode({ state: demoState(), config: demoConfig(), ...over });
const noNetState = () => demoState({ network: null, peers: [], domains: [], ports: { published: [], network: [] } });
const puts = (node) => node.calls.filter((c) => c.method === "PUT" && c.path === "/api/config");
const buttonByText = (wrapper, text) => wrapper.findAll("button").find((b) => b.text().trim() === text);

describe("\"exit via me\" is saved at once", () => {
  it("the switch sends a PUT with the inverted exit.enabled, status \"Saved\"", async () => {
    const node = makeNode({ config: demoConfig({ exit: { enabled: false } }) });
    const { wrapper } = await mountPage(node);
    const toggle = wrapper.get("#field-exit-enabled");
    const before = node.config.exit.enabled;
    expect(typeof before, "was — a boolean value, not null").toBe("boolean");

    await toggle.setValue(!before);
    await settle();

    expect(puts(node)).toHaveLength(1);
    expect(puts(node)[0].body.config.exit.enabled, "was=" + before).toBe(!before);
    expect(node.config.exit.enabled).toBe(!before);
    const status = wrapper.get("#field-exit-enabled-status");
    expect(shown(status.element)).toBe(true);
    expect(status.text()).toBe("Saved");
  });
});

describe("--no-network, \"Join\" with an empty field", () => {
  it("the hint \"Paste an invite…\" is shown in the join card on the first screen, no request", async () => {
    const node = makeNode({ state: noNetState() });
    const { wrapper } = await mountPage(node);
    await wrapper.get("#home-start-join-btn").trigger("click");
    await settle();
    await wrapper.get("#join-key-input").setValue("");
    await wrapper.get("#join-btn").trigger("click");
    await settle();

    expect(wrapper.get("#sec-home").findAll("#join-card")).toHaveLength(1);
    const error = wrapper.get("#join-key-error");
    expect(shown(error.element)).toBe(true);
    expect(error.text()).toBe(EMPTY_KEY_HINT);
    expect(node.calls.filter((c) => c.method === "POST")).toEqual([]);
  });
});

describe("the \"Connect the browser to Mesh Town\" card in #home-proxy", () => {
  const PAC_ADDRESS = "http://" + LISTEN + "/proxy.pac"; // the demo node snapshot has no pac_url field — the address is assembled from listen

  it("home proxy hint (proxy on): the intro line, the PAC address, the \"Your system\" group (one pressed); the manual setup is collapsed and expands: Firefox, 3 browser buttons without Safari — in happy-dom the system is Linux (one pressed), \"Copy address\"", async () => {
    const { wrapper } = await mountPage(makeNode({ state: demoState({ socks: { enabled: true, listen: LISTEN } }) }));
    expect(wrapper.findAll("#home-proxy")).toHaveLength(1);
    const card = wrapper.get("#home-proxy");
    expect(card.get("#home-proxy-title").text()).toBe(PAC_TITLE);
    expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO);
    expect(card.get("#pac-url-text").text()).toBe(PAC_ADDRESS);
    expect(card.findAll("#pac-copy-btn")).toHaveLength(1);
    const systems = card.get('[role="group"]#os-seg').findAll("button");
    expect(systems).toHaveLength(4);
    expect(systems.filter((b) => b.attributes("aria-pressed") === "true")).toHaveLength(1);
    expect(card.find("#proxy-setup").exists(), "the manual block is collapsed").toBe(false);
    expect(card.text()).not.toContain("How to set up");

    await wrapper.get("#manual-toggle").trigger("click");
    await settle();

    expect(card.get("#manual-toggle").attributes("aria-expanded")).toBe("true");
    expect(card.text()).toContain("Firefox");
    expect(card.findAll("#socks-copy-btn")).toHaveLength(1);
    const buttons = card.get('[role="group"][aria-label="Your browser"]').findAll("button");
    expect(buttons, "Chrome, Edge, Firefox: Safari exists only on macOS, and in happy-dom the system is Linux").toHaveLength(3);
    expect(card.find("#browser-safari").exists()).toBe(false);
    expect(buttons.filter((b) => b.attributes("aria-pressed") === "true")).toHaveLength(1);
  });

  it("home proxy hint (proxy off): the card says \"The browser proxy is off\", no PAC address and no manual setup", async () => {
    const { wrapper } = await mountPage(makeNode());

    expect(wrapper.findAll("#home-proxy")).toHaveLength(1);
    expect(wrapper.get("#home-proxy").text()).toContain("The browser proxy is off");
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.find("#proxy-setup").exists()).toBe(false);
  });
});

describe("#exit-list is not rebuilt by the poll", () => {
  it("the candidate's radio button survives 2 poll ticks, a click on \"As usual\" sends route.exits == []", async () => {
    const node = makeNode({ config: demoConfig({ route: { exits: [STRANGER_ID] } }) });
    const { wrapper } = await mountPage(node);
    const choice = () => wrapper.findAll("#exit-list input[type=radio]").filter((r) => r.element.closest("label").textContent.includes(STRANGER_LABEL));
    expect(choice()).toHaveLength(1);
    const marked = choice()[0].element;

    await polls(node, 2);

    expect(choice()).toHaveLength(1);
    expect(choice()[0].element, "the radio button node was rebuilt by the poll").toBe(marked);
    expect(marked.isConnected).toBe(true);

    await wrapper.get("#exit-direct").setValue(true);
    await settle();
    expect(puts(node)).toHaveLength(1);
    expect(puts(node)[0].body.config.route.exits).toEqual([]);
  });
});
