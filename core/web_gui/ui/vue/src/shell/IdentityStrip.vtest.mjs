/* The "node identity is temporary" strip: the class and text from the `identity` prop, hidden without it. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { NET_NAME } from "../common/words.ts";
import IdentityStrip from "./IdentityStrip.vue";
import { identityStrip } from "./status-line.ts";

describe("IdentityStrip", () => {
  describe("identity is temporary, declined at startup", () => {
    it("shown: class declined and the consequence text", () => {
      const state = { node: { identity: "temporary", identity_reason: "declined" } };
      const w = mountStrip(identityStrip(state, null));
      const el = w.get("#identity-strip");
      expect(el.attributes("hidden")).toBeUndefined();
      expect(el.classes()).toContain("identity-strip-declined");
      expect(el.text()).toBe(NET_NAME + " was started in a mode that doesn't remember this computer. After a restart, other members will see this computer as a new member.");
    });
  });

  describe("identity is not temporary", () => {
    it("the strip is hidden and empty", () => {
      const state = { node: { identity: "persistent" } };
      const w = mountStrip(identityStrip(state, null));
      const el = w.get("#identity-strip");
      expect(el.attributes("hidden")).toBeDefined();
      expect(el.text()).toBe("");
    });
  });

  describe("the node sent a reason in the file", () => {
    it("instead of the fallback text — the reason with a capital letter", () => {
      const state = { node: { identity: "temporary", identity_reason: "bad_file" } };
      const w = mountStrip(identityStrip(state, { message: "broken config" }));
      expect(w.get("#identity-strip").classes()).toContain("identity-strip-problem");
      expect(w.get("#identity-strip").text()).toMatch(/^Broken config\. /);
    });
  });
});

function mountStrip(identity) {
  return mount(IdentityStrip, { props: { identity } });
}
