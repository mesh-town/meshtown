/* Texts of the networks block: fixed wording. The test pins the wording: changing a text
 * here is deliberate, together with the snapshots. */

import { describe, expect, it } from "vitest";
import * as texts from "./nets-texts.ts";

describe("nets-texts", () => {
  describe("the \"Which local networks can connect\" block", () => {
    it.each([
      ["NETS_TITLE", "Which local networks can connect"],
      ["NETS_SUB", "Applies to both the proxy and the settings page."],
      ["NETS_TAG", "after restart"],
      ["AUTO_NOTE", "The networks are currently chosen automatically."],
      ["AUTO_BTN", "Return to automatic choice"],
      ["EMPTY_HINT", "Select at least one network."],
      ["SAVED_NOTE", "Saved. Takes effect after Mesh Town restarts."],
      ["ROUTER_RESTART_NOTE", "The page will disappear for a few seconds and reopen on its own."],
    ])("%s", (name, expected) => {
      expect(texts[name]).toBe(expected);
    });
  });

  describe("warnings", () => {
    it("WARN_PROVIDER — the price of the \"ISP network\" checkbox", () => {
      expect(texts.WARN_PROVIDER).toBe(
        "Then any device on the network the router is connected to will be able to use the proxy without a password: for example, on the main home router's network or your ISP's. Select only if that is also your network.",
      );
    });

    it("HERE_NOTE — why the checkbox cannot be cleared for the network the page is opened from", () => {
      expect(texts.HERE_NOTE).toBe("This page is open through this network — you can't deselect it here. Open the page from another network or from this computer itself.");
    });
  });

  describe("setup step 2 (router-only mode)", () => {
    it("PAC_INTRO_ROUTER — about a phone or computer connected to the router", () => {
      expect(texts.PAC_INTRO_ROUTER).toBe(
        "Enter this address once in the proxy settings of a phone or computer connected to the router. After that, the browser opens sites through Mesh Town by itself whenever you choose an exit. No password needed.",
      );
    });

    it("LAN_HINT_MULTI — which address to take when several networks are open", () => {
      expect(texts.LAN_HINT_MULTI).toBe("Use the address of the network the device is connected to.");
    });

    it("ROUTER_ONLY_TEXT — the router's proxy is visible only to the router itself", () => {
      expect(texts.ROUTER_ONLY_TEXT).toBe("The proxy is open only to the router itself — phones and computers at home won't be able to use it.");
    });
  });

  describe("the \"Who can use the proxy\" description", () => {
    it("DESC_SOCKS_ACCESS_NETS — any device from the networks selected below", () => {
      expect(texts.DESC_SOCKS_ACCESS_NETS).toBe(
        "In local network mode the proxy has no password: any device on the networks selected below can use it. Takes effect after Mesh Town restarts.",
      );
    });
  });

  describe("words for a newcomer", () => {
    it("the texts have no localhost, WAN, LAN, \"interface\"", () => {
      const all = Object.values(texts).filter((v) => typeof v === "string").join("\n");
      expect(all).not.toMatch(/localhost|\bWAN\b|\bLAN\b|interface/i);
    });
  });
});
