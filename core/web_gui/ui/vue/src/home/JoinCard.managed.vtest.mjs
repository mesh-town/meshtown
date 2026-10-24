/* The join card with network kinds: the choice of the kind when creating (Peer unless another is chosen) and the line that says
 * what a pasted invite leads to. The node is a stub `api` that records every call; the pause in typing runs on fake timers. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import JoinCard from "./JoinCard.vue";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INVITE = "mesh-invite2-ABCDEFGH";
const INSPECT = "/api/network/inspect";
const CREATE = "/api/network/create";
const MANAGED_LINE = 'Network "work" · Managed: admins decide who joins';
const PEER_LINE = 'Network "work" · Peer: every member can invite';
const MANAGED_STATUS =
  'You created "work" and you\'re its admin. Nobody else is here yet: use Invite to "work" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";
const NO_NET = { state: baseState({ network: null, peers: [] }) };

let mounted;

beforeEach(() => {
  localStorage.clear();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
});
afterEach(() => {
  vi.useRealTimers();
  mounted.wrapper.unmount();
});

describe("creating a network", () => {
  describe("the kind", () => {
    it("is offered in the create form, Peer chosen", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      expect(card.wrapper.find("#create-box #kind-peer").exists(), "the Peer card in the create form").toBe(true);
      expect(card.wrapper.find("#create-box #kind-managed").exists(), "the Managed card in the create form").toBe(true);
      expect(card.wrapper.get("#kind-peer").element.checked).toBe(true);
      expect(card.wrapper.get("#kind-managed").element.checked).toBe(false);
    });

    it("is not offered in the join form", async () => {
      const card = await setup(nodeAnswering());
      expect(card.wrapper.find("#join-box #kind-peer").exists()).toBe(false);
      expect(card.wrapper.find("#join-box #kind-managed").exists()).toBe(false);
    });

    it("the warning is there only while Managed is chosen", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      expect(card.wrapper.find("#kind-managed-warn").exists()).toBe(false);
      await card.when.kindChosen("kind-managed");
      expect(card.wrapper.find("#kind-managed-warn").exists()).toBe(true);
      await card.when.kindChosen("kind-peer");
      expect(card.wrapper.find("#kind-managed-warn").exists()).toBe(false);
    });

    it("the warning names the typed network", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.kindChosen("kind-managed");
      expect(card.wrapper.get("#kind-managed-warn").text()).toContain('"work" can\'t be managed any more');
    });
  });

  describe("Peer, the default", () => {
    it("sends the name alone", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.createClicked();
      expect(card.bodiesTo(CREATE)).toEqual([{ label: "work" }]);
    });

    it("the status after creating is the former short sentence", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.createClicked();
      expect(card.wrapper.get("#join-status").text()).toBe('Network "work" created.');
    });
  });

  describe("Managed chosen", () => {
    it("sends the name and the kind", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.kindChosen("kind-managed");
      await card.when.createClicked();
      expect(card.bodiesTo(CREATE)).toEqual([{ label: "work", kind: "Managed" }]);
    });

    it("then Peer again — the kind is not sent", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.kindChosen("kind-managed");
      await card.when.kindChosen("kind-peer");
      await card.when.createClicked();
      expect(card.bodiesTo(CREATE)).toEqual([{ label: "work" }]);
    });

    it("the status says you are its admin and asks for a second admin", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.nameTyped("work");
      await card.when.kindChosen("kind-managed");
      await card.when.createClicked();
      expect(card.wrapper.get("#join-status").text()).toBe(MANAGED_STATUS);
    });

    it("an empty name is refused as before, nothing is sent", async () => {
      const card = await setup(nodeAnswering());
      await card.when.createOpened();
      await card.when.kindChosen("kind-managed");
      await card.when.createClicked();
      expect(card.wrapper.get("#create-label-error").text()).toBe('Enter a name, for example "family".');
      expect(card.bodiesTo(CREATE)).toEqual([]);
    });
  });
});

describe("a pasted invite", () => {
  it("says nothing and asks nothing before the pause in typing is over", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Managed", id_hex: "ab12" } }));
    await card.when.inviteTyped(INVITE);
    expect(card.wrapper.find("#join-kind-preview").exists()).toBe(false);
    expect(card.bodiesTo(INSPECT)).toEqual([]);
  });

  it("then says that a Managed network is decided by admins", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Managed", id_hex: "ab12" } }));
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    expect(card.wrapper.get("#join-kind-preview").text()).toBe(MANAGED_LINE);
    expect(card.bodiesTo(INSPECT)).toEqual([{ key: INVITE }]);
  });

  it("or that every member of a Peer network can invite", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Peer", id_hex: "ab12" } }));
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    expect(card.wrapper.get("#join-kind-preview").text()).toBe(PEER_LINE);
  });

  it("an emptied field takes the line away", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Managed", id_hex: "ab12" } }));
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    await card.when.inviteTyped("");
    expect(card.wrapper.find("#join-kind-preview").exists()).toBe(false);
  });

  it("does not change the join: the key alone is sent", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Managed", id_hex: "ab12" } }));
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    await card.when.joinClicked();
    expect(card.bodiesTo("/api/network/join")).toEqual([{ key: INVITE }]);
  });

  describe("the node refuses to describe it (422)", () => {
    it("there is no line and no error", async () => {
      const card = await setup(nodeAnswering({ inspect: new ApiError(422, "invalid_key", "Not an invite", { message: "Not an invite" }) }));
      await card.when.inviteTyped(INVITE);
      await card.when.pauseOver();
      expect(card.bodiesTo(INSPECT)).toEqual([{ key: INVITE }]);
      expect(card.wrapper.find("#join-kind-preview").exists()).toBe(false);
      expect(card.wrapper.get("#join-key-error").attributes("hidden")).toBeDefined();
    });
  });
});

describe("the other modes of the card", () => {
  it("with a network a pasted invite is described, and the create form still offers the kind", async () => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Managed", id_hex: "ab12" } }), { state: baseState() });
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    expect(card.wrapper.get("#join-kind-preview").text()).toBe(MANAGED_LINE);
    await card.when.createOpened();
    expect(card.wrapper.get("#kind-peer").element.checked).toBe(true);
  });

  it.each([{ quiet: true }, { removed: true }])("with %o the kind picker and the preview work", async (props) => {
    const card = await setup(nodeAnswering({ inspect: { label: "work", kind: "Peer", id_hex: "ab12" } }), { ...NO_NET, props });
    await card.when.inviteTyped(INVITE);
    await card.when.pauseOver();
    expect(card.wrapper.get("#join-kind-preview").text()).toBe(PEER_LINE);
    await card.when.createOpened();
    expect(card.wrapper.get("#kind-peer").element.checked).toBe(true);
    expect(card.wrapper.find("#kind-managed").exists()).toBe(true);
  });
});

// Preconditions: how the node answers `inspect` (a reply or an `ApiError`; unlisted — a refusal) and what card is mounted (no network by default).
async function setup(answers, detail = NO_NET) {
  api.mockImplementation(async (method, path, body) => {
    const answer = answers(method, path, body);
    if (answer instanceof Error) throw answer;
    return answer;
  });
  mounted = mountSection(JoinCard, detail);
  await flushPromises();
  return cardOf(mounted);
}

function nodeAnswering({ inspect = new ApiError(404, "not_found") } = {}) {
  return (method, path, body) => {
    if (path === INSPECT) return inspect;
    if (path === CREATE) return { key: "mesh1-secret", label: body.label, restart: false };
    if (path === "/api/network/join") return { label: "work", restart: false };
    return new Error("unexpected request " + method + " " + path);
  };
}

function cardOf({ wrapper, store }) {
  const hasNet = !!store.state?.network;
  return {
    wrapper,
    bodiesTo: (path) => api.mock.calls.filter((c) => c[1] === path).map((c) => c[2]),
    when: {
      // Without a network the form is opened by the setup's button; here the card is bare, so its own state is switched.
      async createOpened() {
        if (hasNet) await wrapper.get("#create-toggle").trigger("click");
        else await wrapper.vm.$.setupState.j.openCreate();
        await flushPromises();
      },
      async inviteTyped(value) {
        await wrapper.get("#join-key-input").setValue(value);
        await flushPromises();
      },
      async nameTyped(value) {
        await wrapper.get("#create-label-input").setValue(value);
      },
      // Clicks the card (its label), as a person does: the label forwards the click to the radio.
      async kindChosen(id) {
        wrapper.get("#" + id).element.closest("label").click();
        await flushPromises();
      },
      async pauseOver() {
        await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS + 1);
        await flushPromises();
      },
      async createClicked() {
        await wrapper.get("#create-btn").trigger("click");
        await flushPromises();
      },
      async joinClicked() {
        await wrapper.get("#join-btn").trigger("click");
        await flushPromises();
      },
    },
  };
}
