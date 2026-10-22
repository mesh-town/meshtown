/* The whole "Home" tab on a stub node (moved from e2e: home-island 61, 63, 64; home-netname 32;
 * home-proxy-join 31): the page with the real `api.js`, poller and store. Visibility is checked by the
 * `hidden`/`style`/`open` attributes: happy-dom doesn't compute styles. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shownStrict as shown, visibleText } from "../testing/dom.mjs";

const NET_TITLE = "webcheck-wizard-net";
const SECRET = "mesh-invite2-Zq9xK2mP7vLw4TnB8cRd5YfH3jUe6GaS";
const NO_GARBAGE = /\bnull\b|\bundefined\b/;
const BARE_THROUGH_NETWORK = /through the network(?! Mesh Town)/;
const OTHER_TABS = ["peers", "domains", "network", "log", "settings"];
const LISTEN = "127.0.0.1:1080";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

const makeNode = (over = {}) => createFakeNode({ state: demoState(), config: demoConfig(), ...over });
const noNetState = () => demoState({ network: null, peers: [], domains: [], ports: { published: [], network: [] } });

const buttonByText = (wrapper, text) => wrapper.findAll("button").find((b) => b.text().trim() === text);

function trapClipboard() {
  const writes = [];
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => { writes.push(t); } }, configurable: true });
  return writes;
}

afterEach(() => Reflect.deleteProperty(navigator, "clipboard"));

describe("the Home tab is not rebuilt by polling", () => {
  it("the name field with typed text and the \"As usual\" radio button stay the same nodes", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);
    const field = wrapper.get("#field-node-name");
    const direct = wrapper.get("#exit-direct");
    expect(shown(field.element)).toBe(true);
    field.element.focus();
    await field.setValue("abc");

    await polls(node, 2);

    expect(wrapper.get("#field-node-name").element, "the name field was rebuilt by polling").toBe(field.element);
    expect(wrapper.get("#exit-direct").element, "the \"As usual\" radio button was rebuilt by polling").toBe(direct.element);
    expect(field.element.isConnected).toBe(true);
    expect(field.element.value).toBe("abc");
    expect(document.activeElement).toBe(field.element);
  });
});

describe("the \"Three steps\" setup after creating a network", () => {
  const createRoute = (body, node) => {
    node.state.network = { id_hex: "net-new", masked: "mesh1-••••", label: body.label };
    return { label: body.label };
  };
  const wizard = (wrapper) => wrapper.find('[aria-labelledby="home-wizard-title"]');
  const statePanel = (wrapper) => wrapper.find('[aria-label="How sites open right now"]');

  async function createNet(wrapper, node) {
    await wrapper.get("#home-start-create-btn").trigger("click");
    await wrapper.get("#create-label-input").setValue(NET_TITLE);
    await wrapper.get("#create-btn").trigger("click");
    await settle();
    await polls(node, 1);
  }

  it("step 2, \"Done, my browser is set up\", step 3, \"Done\" — the status panel; after a reload there is no setup", async () => {
    const node = makeNode({ state: noNetState(), routes: { "POST /api/network/create": createRoute } });
    const { wrapper } = await mountPage(node);
    await createNet(wrapper, node);

    expect(wizard(wrapper).text()).toContain("✓ You joined the network");
    const step2 = wizard(wrapper).findAll("h3").find((h) => /Set up your browser/.test(h.text()));
    expect(step2 && shown(step2.element)).toBe(true);
    expect(shown(buttonByText(wizard(wrapper), "Done, my browser is set up").element)).toBe(true);

    await buttonByText(wizard(wrapper), "Done, my browser is set up").trigger("click");
    const radio = wizard(wrapper).get("#exit-direct");
    expect(shown(radio.element)).toBe(true);
    expect(radio.element.closest("label").textContent.trim()).toMatch(/^Directly/);

    await buttonByText(wizard(wrapper), "Done").trigger("click");
    expect(wizard(wrapper).exists()).toBe(false);
    expect(statePanel(wrapper).exists() && shown(statePanel(wrapper).element)).toBe(true);

    unmountPage();
    const again = await mountPage(node);
    expect(wizard(again.wrapper).exists()).toBe(false);
    expect(visibleText(again.wrapper.element)).not.toMatch(NO_GARBAGE);
  });
});

describe("the invite is visible only in the card", () => {
  it("the invite string is not in localStorage/sessionStorage, nor in title, nor in document.title, nor in the console", async () => {
    const consoleCalls = [];
    for (const m of ["log", "info", "warn", "error", "debug"]) vi.spyOn(console, m).mockImplementation((...a) => consoleCalls.push(a.map(String).join(" ")));
    trapClipboard();
    const node = makeNode({ routes: { "GET /api/network/invite": { invite: SECRET } } });
    const { wrapper } = await mountPage(node);

    await wrapper.get("#netkey-invite-btn").trigger("click");
    await settle();
    const value = wrapper.get('[aria-label="Invite to send"]');
    expect(shown(value.element)).toBe(true);
    const secret = value.text().trim();
    expect(secret.length, "the invite string is not empty").toBeGreaterThan(0);

    const stored = [...Object.values(localStorage), ...Object.values(sessionStorage)];
    expect(stored.some((v) => v.includes(secret)), "the secret ended up in storage").toBe(false);
    const titles = [...document.querySelectorAll("[title]")].map((el) => el.getAttribute("title") ?? "");
    expect(titles.some((t) => t.includes(secret)), "the secret is in a title tooltip").toBe(false);
    expect(document.title.includes(secret), "the secret is in document.title").toBe(false);
    expect(consoleCalls.some((t) => t.includes(secret)), "the secret is in the console log").toBe(false);
  });
});

describe("the network name in texts", () => {
  it("the first screen names the network \"Mesh Town\", there is no bare \"through the network\", after cycling the tabs one panel is visible", async () => {
    const { wrapper } = await mountPage(makeNode());
    expect(visibleText(wrapper.get("#sec-home").element)).toContain("Mesh Town");

    for (const section of OTHER_TABS) {
      await openTab(wrapper, section);
      expect(visibleText(wrapper.element), "tab \"" + section + "\"").not.toMatch(BARE_THROUGH_NETWORK);
    }
    const panels = wrapper.findAll('[role="tabpanel"]').filter((p) => shown(p.element));
    expect(panels).toHaveLength(1);
  });
});

describe("the PAC address, proxy address and port are separate values", () => {
  const withProxy = () => makeNode({ state: demoState({ socks: { enabled: true, listen: LISTEN } }) });

  it("home proxy address port split (proxy on): the PAC address is copied right away, the address and port from socks.listen — after \"Not working? Set up manually\", each button copies only its own", async () => {
    const writes = trapClipboard();
    const { wrapper } = await mountPage(withProxy());
    const [host, port] = [LISTEN.slice(0, LISTEN.lastIndexOf(":")), LISTEN.slice(LISTEN.lastIndexOf(":") + 1)];
    const pacUrl = "http://" + LISTEN + "/proxy.pac";
    expect(shown(wrapper.get("#sec-home").element)).toBe(true);
    expect(wrapper.get("#pac-url-text").text()).toBe(pacUrl);
    expect(wrapper.find("#socks-host-text").exists(), "the address and port are only in the manual block").toBe(false);

    await wrapper.get("#pac-copy-btn").trigger("click");
    await settle();
    expect(writes.at(-1)).toBe(pacUrl);

    await wrapper.get("#manual-toggle").trigger("click");
    expect(wrapper.get("#socks-host-text").text()).toBe(host);
    expect(wrapper.get("#socks-port-text").text()).toBe(port);

    writes.length = 0;
    await wrapper.get("#socks-copy-btn").trigger("click");
    await settle();
    expect(writes.at(-1)).toBe(host);
    writes.length = 0;
    await wrapper.get("#socks-copy-port-btn").trigger("click");
    await settle();
    expect(writes.at(-1)).toBe(port);
  });

  it("home proxy address port split (proxy off): no PAC address, no #proxy-setup, no manual setup button", async () => {
    const { wrapper } = await mountPage(makeNode());
    expect(shown(wrapper.get("#sec-home").element)).toBe(true);
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.find("#proxy-setup").exists()).toBe(false);
    expect(wrapper.find("#manual-toggle").exists()).toBe(false);
  });
});
