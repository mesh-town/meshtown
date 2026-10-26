/* `peers/SecondAdminHint.vue`: the "name a second admin" hint above the member list. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import SecondAdminHint from "./SecondAdminHint.vue";
import { SECOND_ADMIN_HINT } from "../members/members-texts.ts";
import { managedFacts } from "../members/managed-ctx.ts";
import { ANNA_FULL, ME_FULL, managedState } from "../testing/managed-fixtures.mjs";

let wrapper;
afterEach(() => wrapper.unmount());

const hintFor = (meRole, admins) => {
  const facts = managedFacts(managedState({ meRole, admins }).network, null, ME_FULL);
  wrapper = mount(SecondAdminHint, { props: { onlyAdmin: !!facts.onlyAdmin } });
  return wrapper.find("#second-admin-hint");
};

describe("SecondAdminHint", () => {
  describe("only admin", () => {
    it("is shown", () => {
      const el = hintFor("admin", [ME_FULL]);
      expect(el.exists()).toBe(true);
      expect(el.text()).toBe(SECOND_ADMIN_HINT);
    });

    it("has no close button", () => {
      hintFor("admin", [ME_FULL]);
      expect(wrapper.find("button").exists()).toBe(false);
    });
  });

  describe("two admins", () => {
    it("is hidden", () => {
      expect(hintFor("admin", [ME_FULL, ANNA_FULL]).exists()).toBe(false);
    });
  });

  describe("member", () => {
    it("is hidden", () => {
      expect(hintFor("member", [ANNA_FULL]).exists()).toBe(false);
    });
  });
});
