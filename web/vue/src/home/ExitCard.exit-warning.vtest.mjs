/* "Exit via…" when the member list says something about the chosen exit: removed from the network, left, or blocked on this
 * computer. The card shows a red alert above the list; a removed member is not offered any more; a blocked one is shown
 * but cannot be picked. The member list is `store.members` (`GET /api/members`); the chosen exit comes from the config. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mount } from "@vue/test-utils";
import { createStore, STORE_KEY } from "../core/store.ts";
import { PEER2_FULL, PEER_FULL, baseConfig, baseState } from "../testing/section.mjs";
import { applySnapshot } from "../testing/snapshot.mjs";
import { excludedInfo, membersReply, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";
import ExitCard from "./ExitCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("ExitCard warning about the chosen exit", () => {
  describe("the exit was removed from the network", () => {
    it("an alert above the list says so", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: removed() }) });
      const warning = wrapper.get("#exit-warning");
      expect(warning.text()).toBe("peer-one was removed from the network. Choose another exit below.");
      expect(warning.attributes("role")).toBeUndefined();
      expect(warning.classes()).toContain("alert");
    });

    it("the alert comes before the list of exits", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: removed() }) });
      const order = [...wrapper.element.querySelectorAll("#exit-warning, #exit-list")].map((el) => el.id);
      expect(order).toEqual(["exit-warning", "exit-list"]);
    });

    it("the removed member is not offered, the others are", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: removed() }) });
      expect(choiceIds(wrapper)).toEqual([PEER2_FULL]);
    });

    it("\"As usual\" is still there to pick", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: removed() }) });
      expect(wrapper.get("#exit-direct").attributes("disabled")).toBeUndefined();
    });
  });

  describe("the exit left the network on its own", () => {
    it("the alert says it left, and it is not offered", () => {
      const left = removed({ denied_by_me: "left", excluded: excludedInfo({ how: "left", voters: [] }) });
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: left }) });
      expect(wrapper.get("#exit-warning").text()).toBe("peer-one left the network. Choose another exit below.");
      expect(choiceIds(wrapper)).toEqual([PEER2_FULL]);
    });
  });

  describe("the exit is blocked on this computer", () => {
    it("the alert says it is blocked", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: blocked() }) });
      expect(wrapper.get("#exit-warning").text()).toBe("peer-one is blocked on this computer. Choose another exit below.");
      expect(wrapper.get("#exit-warning").attributes("role")).toBeUndefined();
    });

    it("the row stays in the list with the words \"Blocked on this computer\" instead of the connection", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: blocked() }) });
      expect(choiceIds(wrapper)).toEqual([PEER_FULL, PEER2_FULL]);
      expect(wrapper.get(`.exit-meta[data-id="${PEER_FULL}"]`).text()).toBe("Blocked on this computer");
    });

    it("its radio button is disabled, the other one is not", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: blocked() }) });
      expect(radio(wrapper, PEER_FULL).disabled).toBe(true);
      expect(radio(wrapper, PEER2_FULL).disabled).toBe(false);
    });
  });

  describe("a member who is not the chosen exit", () => {
    it("blocked: its radio is disabled, and there is no alert", () => {
      const { wrapper } = mountExit({ chosen: [], members: members({ one: blocked() }) });
      expect(radio(wrapper, PEER_FULL).disabled).toBe(true);
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
    });

    it("removed: it is not in the list, and there is no alert", () => {
      const { wrapper } = mountExit({ chosen: [], members: members({ one: removed() }) });
      expect(choiceIds(wrapper)).toEqual([PEER2_FULL]);
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
    });
  });

  describe("nothing to warn about", () => {
    it("the chosen exit is an ordinary member: no alert, nothing disabled", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members() });
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
      expect(wrapper.findAll("input.exit-choice").map((i) => i.element.disabled)).toEqual([false, false]);
    });

    it("this computer itself was removed (me_excluded): no alert about the exit", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: members({ one: removed(), meExcluded: true }) });
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
    });

    it("the node has no member list (an older node): the list is whole, no alert", () => {
      const { wrapper } = mountExit({ chosen: [PEER_FULL], members: null });
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
      expect(choiceIds(wrapper)).toEqual([PEER_FULL, PEER2_FULL]);
    });
  });

  describe("the member list changes while the card is open", () => {
    it("the alert appears when the list says the exit is gone, and goes when it is back", async () => {
      const { wrapper, store } = mountExit({ chosen: [PEER_FULL], members: members() });
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
      store.members = members({ one: removed() });
      await nextTick();
      expect(wrapper.get("#exit-warning").text()).toBe("peer-one was removed from the network. Choose another exit below.");
      store.members = members();
      await nextTick();
      expect(wrapper.find("#exit-warning").exists()).toBe(false);
      expect(choiceIds(wrapper)).toEqual([PEER_FULL, PEER2_FULL]);
    });
  });
});

const removed = (over = {}) => peerOneRow({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo(), ...over });
const blocked = () => peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" });

// The member list of the three nodes of `baseState`; `one` replaces the row of peer-one.
function members({ one = peerOneRow(), meExcluded = false } = {}) {
  return membersReply({ rows: [selfRow(), one, peerTwoRow({ name: "peer-two", online: true })], me_excluded: meExcluded });
}

// Two members offer their connection as an exit: peer-one and peer-two.
function twoExitsState() {
  const peers = baseState().peers.map((p) => (p.id_full === PEER2_FULL ? { ...p, name: "peer-two", online: true, exit: true } : p));
  return baseState({ peers });
}

function mountExit({ chosen, members: list }) {
  const store = createStore();
  applySnapshot(store, { state: twoExitsState(), config: baseConfig({ route: { exits: chosen } }), configRev: 1 });
  store.members = list;
  const wrapper = mount(ExitCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return (mounted = { wrapper, store });
}

const choiceIds = (wrapper) => wrapper.findAll("input.exit-choice").map((i) => i.attributes("data-id"));
const radio = (wrapper, idFull) => wrapper.get(`input.exit-choice[data-id="${idFull}"]`).element;
