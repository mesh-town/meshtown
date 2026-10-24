/* The setup's first step with network kinds: "Create a new network" opens the create form with the kind chosen as Peer. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { baseState, mountSection } from "../testing/section.mjs";
import SetupWizard from "./SetupWizard.vue";

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("SetupWizard > step 1, no network > the kind of the new network", () => {
  describe("before a choice", () => {
    it("no kind is offered", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.find("#kind-peer").exists()).toBe(false);
      expect(wrapper.find("#kind-managed").exists()).toBe(false);
    });
  });

  describe("after \"Create a new network\"", () => {
    it("the picker is in the create form with Peer chosen", async () => {
      const { wrapper } = await createOpened();
      expect(wrapper.find("#create-box #kind-peer").exists(), "the Peer card in the create form").toBe(true);
      expect(wrapper.find("#create-box #kind-managed").exists(), "the Managed card in the create form").toBe(true);
      expect(wrapper.get("#kind-peer").element.checked).toBe(true);
      expect(wrapper.get("#kind-managed").element.checked).toBe(false);
    });

    it("the warning about Managed is not shown yet", async () => {
      const { wrapper } = await createOpened();
      expect(wrapper.find("#kind-managed-warn").exists()).toBe(false);
    });
  });

  describe("after \"Join with an invite\"", () => {
    it("the picker is not in the join form", async () => {
      const { wrapper } = mountWizard();
      await wrapper.get("#home-start-join-btn").trigger("click");
      await nextTick();
      expect(wrapper.find("#join-box #kind-peer").exists()).toBe(false);
      expect(wrapper.find("#join-box #kind-managed").exists()).toBe(false);
    });
  });
});

async function createOpened() {
  const res = mountWizard();
  await res.wrapper.get("#home-start-create-btn").trigger("click");
  await nextTick();
  return res;
}

function mountWizard() {
  return (mounted = mountSection(SetupWizard, { state: baseState({ network: null, peers: [] }) }));
}
