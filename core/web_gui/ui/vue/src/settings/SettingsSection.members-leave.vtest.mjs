/* "Leave network" in Settings, in the card with the network next to "Stop": a block `#settings-leave-row` (a title, a hint and the
 * button `#settings-leave-btn` "Leave network…") that opens the same confirmation dialog as the Members tab. The line under it,
 * `p#settings-leave-status[role=status]`, shows the result and stays after the block itself is gone. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";
import { ApiError, api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { shown } from "../testing/dom.mjs";
import { membersReply, withMembersSummary } from "../testing/members-fixtures.mjs";
import SettingsSection from "./SettingsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
const NET = { id_hex: "net-aaaa", label: "demo-network", masked: "demo-network--••••" };
const DIALOG = 'dialog[aria-labelledby="confirm-title"]';
let mounted;

afterEach(() => mounted.wrapper.unmount());

describe("Settings: leaving the network", () => {
  describe("the computer is in a network", () => {
    it("the block is visible in the network card, after the Stop button", () => {
      const { wrapper } = mountSettings();
      expect(isShown(wrapper, "#settings-leave-row")).toBe(true);
      expect(wrapper.get("#settings-net-card").find("#settings-leave-row").exists()).toBe(true);
      const order = [...wrapper.element.querySelectorAll("#settings-leave-row, #stop-node-btn")].map((el) => el.id);
      expect(order).toEqual(["stop-node-btn", "settings-leave-row"]);
    });

    it("says what leaving does and names the network", () => {
      const { wrapper } = mountSettings();
      const row = wrapper.get("#settings-leave-row");
      expect(row.text()).toContain("Leave network");
      expect(row.get(".hint").text()).toBe("Disconnect this computer from \"demo-network\" for good. You'll need a new invite to come back.");
    });

    it("a network with no name: the hint says \"the network\"", () => {
      const { wrapper } = mountSettings({ network: { id_hex: "net-aaaa", masked: "mesh1-••••-••••" } });
      expect(wrapper.get("#settings-leave-row .hint").text()).toBe("Disconnect this computer from the network for good. You'll need a new invite to come back.");
    });

    it("the button is a danger button \"Leave network…\"", () => {
      const { wrapper } = mountSettings();
      const button = wrapper.get("#settings-leave-btn");
      expect(button.text()).toBe("Leave network…");
      expect(button.classes()).toEqual(expect.arrayContaining(["btn", "btn-danger"]));
      expect(button.element.tagName).toBe("BUTTON");
    });

    it("the line for the result exists from the start, is role=status and empty", () => {
      const { wrapper } = mountSettings();
      expect(wrapper.get("#settings-leave-status").element.tagName).toBe("P");
      expect(wrapper.get("#settings-leave-status").attributes("role")).toBe("status");
      expect(wrapper.get("#settings-leave-status").text()).toBe("");
    });
  });

  describe("the block is hidden", () => {
    it("when there is no network", () => {
      const { wrapper } = mountSettings({ network: null, peers: [] });
      expect(isShown(wrapper, "#settings-leave-row")).toBe(false);
      expect(wrapper.find("#settings-leave-btn").exists()).toBe(false);
    });

    it("when this computer was removed from the network (the state says so)", () => {
      const { wrapper } = mountSettings(withMembersSummary(baseState({ network: NET }), { me_excluded: true }));
      expect(isShown(wrapper, "#settings-leave-row")).toBe(false);
    });

    it("when this computer was removed (only the member list says so)", async () => {
      const { wrapper, store } = mountSettings();
      store.members = membersReply({ me_excluded: true });
      await nextTick();
      expect(isShown(wrapper, "#settings-leave-row")).toBe(false);
    });
  });

  describe("\"Leave network…\" is clicked", () => {
    it("asks first: the dialog names the network and focuses Cancel; nothing is sent", async () => {
      const { wrapper } = mountSettings();
      await clickLeave(wrapper);
      expect(wrapper.get(DIALOG).element.hasAttribute("open")).toBe(true);
      expect(wrapper.get("#confirm-title").text()).toBe("Leave the network \"demo-network\"?");
      expect(wrapper.get("#member-confirm-yes").text()).toBe("Leave network");
      expect(document.activeElement).toBe(wrapper.get("#member-confirm-cancel").element);
      expect(api).not.toHaveBeenCalled();
    });

    it("Cancel sends nothing and leaves the result line empty", async () => {
      const { wrapper } = mountSettings();
      await clickLeave(wrapper);
      await wrapper.get("#member-confirm-cancel").trigger("click");
      await flushPromises();
      expect(wrapper.get(DIALOG).element.hasAttribute("open")).toBe(false);
      expect(api).not.toHaveBeenCalled();
      expect(wrapper.get("#settings-leave-status").text()).toBe("");
    });

    it("confirming sends the confirmation to the leave endpoint", async () => {
      api.mockResolvedValue(membersReply());
      const { wrapper } = mountSettings();
      await clickLeave(wrapper);
      await wrapper.get("#member-confirm-yes").trigger("click");
      await flushPromises();
      expect(api.mock.calls).toEqual([["POST", "/api/members/leave", { confirm: true }]]);
    });

    it("the line under the block says that you left the network", async () => {
      api.mockResolvedValue(membersReply());
      const { wrapper } = mountSettings();
      await clickLeave(wrapper);
      await wrapper.get("#member-confirm-yes").trigger("click");
      await flushPromises();
      expect(wrapper.get("#settings-leave-status").text()).toBe("You left the network \"demo-network\".");
    });

    it("the line stays after the block goes away", async () => {
      api.mockResolvedValue(membersReply());
      const { wrapper, store } = mountSettings();
      await clickLeave(wrapper);
      await wrapper.get("#member-confirm-yes").trigger("click");
      await flushPromises();
      store.state = withMembersSummary(baseState({ network: NET }), { me_excluded: true });
      await nextTick();
      expect(isShown(wrapper, "#settings-leave-row")).toBe(false);
      expect(wrapper.get("#settings-leave-status").text()).toBe("You left the network \"demo-network\".");
    });

    it("the node refuses: its own sentence is shown under the block", async () => {
      api.mockRejectedValue(new ApiError(409, "busy", "busy", { error: "busy", message: "The node is not in a network" }));
      const { wrapper } = mountSettings();
      await clickLeave(wrapper);
      await wrapper.get("#member-confirm-yes").trigger("click");
      await flushPromises();
      expect(wrapper.get("#settings-leave-status").text()).toBe("The node is not in a network");
      expect(isShown(wrapper, "#settings-leave-row")).toBe(true);
    });
  });

  describe("the node answers with the networks that remain", () => {
    it("the store takes `nets` from the reply", async () => {
      api.mockResolvedValue(leaveReply({ nets: [netBrief("k7q2xa", "my devices")] }));
      const { wrapper, store } = mountSettings();
      await confirmLeave(wrapper);
      expect(store.nets).toEqual([netBrief("k7q2xa", "my devices")]);
    });

    it("the line keeps the sentence it always had", async () => {
      api.mockResolvedValue(leaveReply({ nets: [netBrief("k7q2xa", "my devices")] }));
      const { wrapper } = mountSettings();
      await confirmLeave(wrapper);
      expect(wrapper.get("#settings-leave-status").text()).toBe("You left the network \"demo-network\".");
    });

    it("a leave the node announced to nobody: the line adds a second sentence", async () => {
      api.mockResolvedValue(leaveReply({ announced: false }));
      const { wrapper } = mountSettings();
      await confirmLeave(wrapper);
      const text = wrapper.get("#settings-leave-status").text();
      expect(text.startsWith("You left the network \"demo-network\". ")).toBe(true);
      expect(text).toContain("Mesh Town couldn't let the other members know");
    });

    it("the node did not leave (`left:false`): the store keeps its networks and the line stays empty", async () => {
      const before = [netBrief("main", "demo-network")];
      api.mockResolvedValue(leaveReply({ left: false, nets: [] }));
      const { wrapper, store } = mountSettings();
      store.nets = before;
      await confirmLeave(wrapper);
      expect(store.nets).toEqual(before);
      expect(wrapper.get("#settings-leave-status").text()).toBe("");
    });
  });
});

function mountSettings(over = {}) {
  const state = over.node ? over : baseState({ network: NET, ...over });
  const config = baseConfig({ socks: { enabled: true, port: 1080 } });
  return (mounted = mountSection(SettingsSection, { state, config, configMeta: META, configFile: { writable: true } }));
}

async function clickLeave(wrapper) {
  await wrapper.get("#settings-leave-btn").trigger("click");
  await flushPromises();
  await nextTick();
}

const isShown = (wrapper, selector) => wrapper.find(selector).exists() && shown(wrapper.get(selector).element);

async function confirmLeave(wrapper) {
  await clickLeave(wrapper);
  await wrapper.get("#member-confirm-yes").trigger("click");
  await flushPromises();
}

// The reply of `members/leave`: the member list plus what the node did and the networks that remain.
const leaveReply = (over = {}) => ({ ...membersReply(), left: true, announced: true, nets: [], ...over });

const netBrief = (id, label) => ({
  id, label, id_hex: id + "-hex", status: "online", exit_enabled: false, proxy: false, route_state: "direct", exits: [],
  members_voting: 0, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 1, peers_total: 2,
});
