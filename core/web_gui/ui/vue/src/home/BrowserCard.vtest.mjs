/* "Connect the browser to Mesh Town": the intro line, the auto-config (PAC) address, the line for the network,
 * the manual block collapsed by default, and the footer. The system switch and copying the PAC address are in
 * `BrowserCard.pac.vtest.mjs`. Texts come from `common/pac-texts.ts`. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { MANUAL_BROWSER_HINT_NO_SAFARI, MANUAL_INTRO_BASE, MANUAL_TAIL_LINUX, MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN, PAC_ADDR_LABEL, PAC_INTRO, PAC_NO_EXIT_NOTE, PAC_TITLE, PAC_WHY, WIZARD_AGAIN } from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";
import { TERM_TIPS } from "./wizard/term-hints.ts";

const LISTEN = "127.0.0.1:1080";
const PAC_URL = "http://127.0.0.1:1081/proxy.pac"; // the port is deliberately not the listen one: the address is taken from the field, not assembled
const PROXY_ON = { socks: { enabled: true, listen: LISTEN, pac_url: PAC_URL } };
const OFF_HINT = "The browser proxy is off — turn it on in \"Settings\"";
const SETUP_MARK = "meshtown.homeSetup";
let mounted;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("meshtown.proxyScope", "system"); // the scope question is answered; its own tests are in BrowserCard.scope.vtest.mjs
});
afterEach(() => mounted.wrapper.unmount());

describe("BrowserCard", () => {
  describe("proxy on", () => {
    it("a region card titled \"Connect the browser to Mesh Town\", with the intro line under it", () => {
      const { wrapper } = mountBrowser();
      const card = wrapper.get("#home-proxy");
      expect(card.attributes("role")).toBe("region");
      expect(card.attributes("aria-labelledby")).toBe("home-proxy-title");
      expect(card.classes()).toContain("card");
      expect(wrapper.get("#home-proxy-title").text()).toBe(PAC_TITLE);
      expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO);
    });

    describe("auto-config address", () => {
      it("is taken from socks.pac_url and sits in a row with a label and the \"Copy address\" button", () => {
        const row = mountBrowser().wrapper.get("#pac-row");
        expect(textWithoutTips(row)).toContain(PAC_ADDR_LABEL);
        expect(row.get("#pac-url-text").text()).toBe(PAC_URL);
        expect(row.get("#pac-copy-btn").text()).toBe("Copy address");
      });

      it("the label names the term PAC: a focusable word with a tooltip that explains it", () => {
        const row = mountBrowser().wrapper.get("#pac-row");
        const term = row.get("span.term");
        expect(term.text().startsWith("PAC"), "the term is the word PAC").toBe(true);
        expect(term.attributes("tabindex")).toBe("0");
        const tip = term.get('[role="tooltip"]');
        expect(tip.text()).toBe(TERM_TIPS.PAC);
        expect(term.attributes("aria-describedby")).toBe(tip.attributes("id"));
        expect(textWithoutTips(row)).toContain(PAC_ADDR_LABEL);
      });

      it("no pac_url field in the snapshot — http://<listen>/proxy.pac", () => {
        const { wrapper } = mountBrowser(baseState({ socks: { enabled: true, listen: LISTEN } }));
        expect(wrapper.get("#pac-url-text").text()).toBe("http://127.0.0.1:1080/proxy.pac");
      });

      it("notes beside it: why the address, and what happens while no exit is chosen", () => {
        const { wrapper } = mountBrowser();
        expect(wrapper.get("#pac-why").text()).toBe(PAC_WHY);
        expect(wrapper.get("#pac-no-exit").text()).toBe(PAC_NO_EXIT_NOTE);
      });

      it("an exit is chosen — there is no \"As usual, without …\" line", () => {
        const { wrapper } = mountBrowser(baseState(PROXY_ON), {}, baseConfig({ route: { exits: ["cccc3333"] } }));
        expect(wrapper.find("#pac-no-exit").exists()).toBe(false);
        expect(wrapper.find("#pac-why").exists()).toBe(true);
      });
    });

    describe("the \"For other devices on the network\" line", () => {
      it("it is absent without access from the network", () => {
        expect(mountBrowser().wrapper.find("#proxy-lan").exists()).toBe(false);
      });

      it("with lan — the address with the interface and a \"Copy\" button", () => {
        const { wrapper } = mountBrowser(lanState([{ addr: "192.168.1.7:1080", iface: "br-lan" }]));
        expect(wrapper.get("#proxy-lan").text()).toContain("For other devices on the network: address 192.168.1.7:1080 (br-lan), type HTTP or SOCKS5");
        expect(wrapper.find("#proxy-lan-hint").exists()).toBe(false);
        expect(wrapper.get("#socks-copy-lan-btn").text()).toBe("Copy");
        expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO);
      });

      it("several addresses — all with interface names and a hint about a single network", () => {
        const lan = [{ addr: "192.168.8.1:1080", iface: "br-lan" }, { addr: "10.0.0.5:1080", iface: "eth1" }];
        const { wrapper } = mountBrowser(lanState(lan));
        expect(wrapper.get("#proxy-lan").text()).toContain("192.168.8.1:1080 (br-lan) · 10.0.0.5:1080 (eth1)");
        expect(wrapper.get("#proxy-lan-hint").text()).toContain("first three numbers");
      });
    });

    describe("manual block collapsed", () => {
      it("there is a \"Not working? Set up manually ▸\" button with aria-expanded=false, no steps", () => {
        const { wrapper } = mountBrowser();
        const toggle = wrapper.get("#manual-toggle");
        expect(toggle.text()).toBe(MANUAL_TOGGLE);
        expect(toggle.attributes("aria-expanded")).toBe("false");
        expect(wrapper.find("#proxy-setup").exists()).toBe(false);
      });

      it("the old \"How to set up ▸\" and \"Collapse\" are absent both collapsed and expanded", async () => {
        const { wrapper } = mountBrowser();
        expect(wrapper.text()).not.toContain("How to set up");
        await wrapper.get("#manual-toggle").trigger("click");
        expect(wrapper.text()).not.toContain("How to set up");
        expect(wrapper.text()).not.toContain("Collapse");
      });
    });

    describe("manual block expanded by a click", () => {
      it("aria-expanded=true, the button text with a ▾ arrow, inside the intro line and the manual steps", async () => {
        const { wrapper } = await mountExpanded();
        const toggle = wrapper.get("#manual-toggle");
        expect(toggle.attributes("aria-expanded")).toBe("true");
        expect(toggle.text()).toBe(MANUAL_TOGGLE_OPEN);
        const setup = wrapper.get("#proxy-setup");
        // in happy-dom the system is detected as Linux (userAgent without "Macintosh", "Windows" and "Android")
        expect(introText(setup)).toBe(MANUAL_INTRO_BASE + " " + MANUAL_TAIL_LINUX);
        expect(setup.find("#browser-seg").exists()).toBe(true);
        expect(setup.find("#proxy-steps").exists()).toBe(true);
        expect(setup.get("#proxy-browser-hint").text()).toBe(MANUAL_BROWSER_HINT_NO_SAFARI); // there is no Safari button on Linux
      });

      it("a space between the label and the step value", async () => {
        const { wrapper } = await mountExpanded();
        const texts = wrapper.findAll("#proxy-steps li").map((li) => li.text());
        expect(texts).toContain("SOCKS host: 127.0.0.1 Copy address"); // the browser in the test is Firefox
        expect(texts).toContain("Port: 1080 Copy port");
      });

      it("a second click on the button collapses the block back, the label with the ▸ arrow again", async () => {
        const { wrapper } = await mountExpanded();
        await wrapper.get("#manual-toggle").trigger("click");
        expect(wrapper.get("#manual-toggle").text()).toBe(MANUAL_TOGGLE);
        expect(wrapper.find("#proxy-setup").exists()).toBe(false);
        expect(wrapper.get("#manual-toggle").attributes("aria-expanded")).toBe("false");
      });

      it("address and port are separate values with \"Copy address\" and \"Copy port\" buttons", async () => {
        const { wrapper } = await mountExpanded();
        expect(wrapper.get("#socks-copy-btn").text()).toBe("Copy address");
        expect(wrapper.get("#socks-copy-port-btn").text()).toBe("Copy port");
        expect(wrapper.get("#socks-host-text").text()).toBe("127.0.0.1");
        expect(wrapper.get("#socks-port-text").text()).toBe("1080");
      });

      it("the clipboard note is hidden before copying", async () => {
        const { wrapper } = await mountExpanded();
        expect(wrapper.get("#proxy-copy-note").isVisible()).toBe(false);
      });

      it("choosing a browser marks the button aria-pressed", async () => {
        const { wrapper } = await mountExpanded();
        await wrapper.get("#browser-firefox").trigger("click");
        expect(wrapper.get("#browser-firefox").attributes("aria-pressed")).toBe("true");
        expect(wrapper.get("#browser-chrome").attributes("aria-pressed")).toBe("false");
      });
    });

    describe("footer", () => {
      it("\"Go through the steps in the setup\" sets the setup mark \"browser\"", async () => {
        const { wrapper } = mountBrowser();
        const again = wrapper.findAll("button").find((b) => b.text() === WIZARD_AGAIN);
        expect(again, "no \"" + WIZARD_AGAIN + "\" button").toBeDefined();
        await again.trigger("click");
        expect(localStorage.getItem(SETUP_MARK)).toBe("browser");
      });
    });
  });

  describe("proxy off", () => {
    it("a line \"The browser proxy is off\" with a link to \"Settings\", the header is unchanged", () => {
      const { wrapper } = mountBrowser(baseState({ socks: { enabled: false, listen: null } }));
      expect(wrapper.get("#home-proxy-title").text()).toBe(PAC_TITLE);
      expect(wrapper.get("#proxy-hint").text()).toContain(OFF_HINT);
      expect(wrapper.get("#proxy-hint a").attributes("href")).toBe("#settings");
      expect(wrapper.get("#proxy-hint").text()).not.toContain(PAC_INTRO);
    });

    it("no PAC address, no system, no manual setup", () => {
      const { wrapper } = mountBrowser(baseState({ socks: { enabled: false, listen: null } }));
      for (const id of ["#pac-row", "#os-seg", "#manual-toggle", "#proxy-setup"]) {
        expect(wrapper.find(id).exists(), id).toBe(false);
      }
    });
  });
});

function lanState(lan) {
  return baseState({ socks: { enabled: true, listen: LISTEN, access: "localnet", lan } });
}

async function mountExpanded() {
  const card = mountBrowser();
  await card.wrapper.get("#manual-toggle").trigger("click");
  return card;
}

function mountBrowser(state = baseState(PROXY_ON), props = {}, config = baseConfig()) {
  const store = createStore();
  applySnapshot(store, { state, config, configRev: 1 });
  const wrapper = mount(BrowserCard, { props, attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return (mounted = { wrapper, store });
}

// The visible text of an element without the tooltip texts of its terms.
function textWithoutTips(wrapper) {
  const copy = wrapper.element.cloneNode(true);
  copy.querySelectorAll('[role="tooltip"]').forEach((tip) => tip.remove());
  return copy.textContent.replace(/\s+/g, " ").trim();
}

// The intro text without the tooltip texts of its terms.
function introText(scope) {
  const copy = scope.get("#proxy-manual-intro").element.cloneNode(true);
  copy.querySelectorAll('[role="tooltip"]').forEach((tip) => tip.remove());
  return copy.textContent.trim();
}
