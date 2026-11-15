/* "More options" under the setup instructions: collapsed by default, holds the manual
 * SOCKS5 block, which depends on the browser proxy being on. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { baseConfig, baseState, mountSection } from "../../testing/section.mjs";
import MoreOptions from "./MoreOptions.vue";

vi.mock("../../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
const PROXY_OFF = { socks: { enabled: false, listen: null, pac_url: null } };
let mounted;

beforeEach(() => {
  localStorage.clear();
  Object.defineProperty(navigator, "userAgent", { value: MAC_CHROME, configurable: true });
});
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("MoreOptions", () => {
  describe("proxy on, macOS", () => {
    it("starts collapsed under \"More options\"", () => {
      const { wrapper } = setup(PROXY_ON);
      const details = wrapper.get("details#more-options");
      expect(details.attributes("open")).toBeUndefined();
      expect(details.get("summary").text()).toBe("More options");
    });

    it("holds the manual-setup toggle and no strict-mode question", () => {
      const { wrapper } = setup(PROXY_ON);
      const details = wrapper.get("details#more-options");
      expect(details.find("#strict-choice").exists(), "strict choice moved out").toBe(false);
      expect(details.find("#manual-toggle").exists(), "manual toggle inside").toBe(true);
    });

    it("the manual block has no browser switcher: the browser comes from the earlier step", async () => {
      const { wrapper } = setup(PROXY_ON);
      await wrapper.get("#manual-toggle").trigger("click");
      expect(wrapper.find("#proxy-setup").exists(), "the manual block is open").toBe(true);
      expect(wrapper.find("#browser-seg").exists()).toBe(false);
      expect(wrapper.find("#proxy-browser-hint").exists()).toBe(false);
    });
  });

  describe("proxy off", () => {
    it("has neither the manual toggle nor the strict-mode question", () => {
      const { wrapper } = setup(PROXY_OFF);
      expect(wrapper.find("#manual-toggle").exists()).toBe(false);
      expect(wrapper.find("#strict-choice").exists()).toBe(false);
    });
  });
});

function setup(socks) {
  return (mounted = mountSection(MoreOptions, { state: baseState(socks), config: baseConfig({ socks: { enabled: socks.socks.enabled, port: 1080 } }) }));
}
