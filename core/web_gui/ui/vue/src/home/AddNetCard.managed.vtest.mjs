/* "Add another network" with network kinds: the choice of the kind when creating, and the line that says what a pasted
 * invite leads to. Same fake node as in `AddNetCard.vtest.mjs`: two networks and one empty slot handed out by `POST /api/nets`. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../core/api.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { shown, visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { settle } from "../testing/page.mjs";
import { baseState } from "../testing/section.mjs";
import AddNetCard from "./AddNetCard.vue";
import { resetNetBanner, useNetBanner } from "./add-net-banner.ts";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

const FRESH = "neta22";
const INVITE = "mesh1-ABCDEFGH";
const CREATE_TOGGLE = "No invite? Create a new network ▸";
const INSPECT = "/api/network/inspect";
const MANAGED_LINE = 'Network "work" · Managed: admins decide who joins';
const PEER_LINE = 'Network "work" · Peer: every member can invite';
const MANAGED_BANNER =
  'You created "work" and you\'re its admin. Nobody else is here yet: use Invite to "work" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";

let mounted;

beforeEach(() => {
  sessionStorage.clear();
  location.hash = "";
  resetNetBanner();
});
afterEach(() => {
  vi.useRealTimers();
  mounted.wrapper.unmount();
  location.hash = "";
  expect(mounted.node.unexpected).toEqual([]);
});

describe("creating a network", () => {
  describe("the kind", () => {
    it("is offered, Peer chosen, once the form is in create mode", async () => {
      const card = await setup();
      await card.when.clicked(CREATE_TOGGLE);
      expect(card.exists("#kind-peer"), "the Peer card").toBe(true);
      expect(card.exists("#kind-managed"), "the Managed card").toBe(true);
      expect(card.get("#kind-peer").element.checked).toBe(true);
      expect(card.get("#kind-managed").element.checked).toBe(false);
      expect(card.text()).toContain("Who can invite and remove members?");
    });

    it("is not offered in join mode", async () => {
      const card = await setup();
      expect(card.exists("#kind-peer")).toBe(false);
      expect(card.exists("#kind-managed")).toBe(false);
      expect(card.text()).not.toContain("Who can invite and remove members?");
    });

    it("goes away with the way back to join mode", async () => {
      const card = await setup();
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.clicked("Have an invite? Join a network ▸");
      expect(card.exists("#kind-managed")).toBe(false);
    });

    it("Managed brings the warning with the typed name, Peer takes it away", async () => {
      const card = await setup();
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.typed("work");
      expect(card.exists("#kind-managed-warn")).toBe(false);
      await card.when.kindChosen("kind-managed");
      expect(card.get("#kind-managed-warn").text()).toContain('"work" can\'t be managed any more');
      await card.when.kindChosen("kind-peer");
      expect(card.exists("#kind-managed-warn")).toBe(false);
    });
  });

  describe("Peer, the default", () => {
    it("sends the name alone", async () => {
      const card = await setup();
      answersCreate(card.node);
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.typed("work");
      await card.when.clicked("Create network");
      expect(card.postedBodies()).toEqual([{}, { label: "work" }]);
    });
  });

  describe("Managed chosen", () => {
    it("sends the name and the kind to the slot's own address", async () => {
      const card = await setup();
      answersCreate(card.node);
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.typed("work");
      await card.when.kindChosen("kind-managed");
      await card.when.clicked("Create network");
      expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/create"]);
      expect(card.postedBodies()).toEqual([{}, { label: "work", kind: "Managed" }]);
      expect(card.wrapper.emitted("done")).toEqual([["created", FRESH]]);
    });

    it("then Peer again — the kind is not sent", async () => {
      const card = await setup();
      answersCreate(card.node);
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.typed("work");
      await card.when.kindChosen("kind-managed");
      await card.when.kindChosen("kind-peer");
      await card.when.clicked("Create network");
      expect(card.postedBodies()).toEqual([{}, { label: "work" }]);
    });

    it("leaves a banner on the new tab: you are its admin", async () => {
      const card = await setup();
      answersCreate(card.node);
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.typed("work");
      await card.when.kindChosen("kind-managed");
      await card.when.clicked("Create network");
      expect(useNetBanner().banner.value).toEqual({ slot: FRESH, text: MANAGED_BANNER });
    });

    it("an empty name is refused as before, nothing is sent", async () => {
      const card = await setup();
      await card.when.clicked(CREATE_TOGGLE);
      await card.when.kindChosen("kind-managed");
      await card.when.clicked("Create network");
      expect(card.alerts()).toEqual(['Enter a name, for example "family".']);
      expect(card.postedPaths()).toEqual([]);
    });
  });
});

describe.each(["peers", "settings"])("a pasted invite in the %s context", (context) => {
  beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));

  it("says nothing and asks nothing before the pause in typing is over", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Managed", id_hex: "ab12" });
    await card.when.typed(INVITE);
    expect(card.exists("#add-net-kind-preview")).toBe(false);
    expect(card.inspectCount()).toBe(0);
  });

  it("then says that a Managed network is decided by admins", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Managed", id_hex: "ab12" });
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    expect(card.get("#add-net-kind-preview").text()).toBe(MANAGED_LINE);
    expect(card.inspectBodies()).toEqual([{ key: INVITE }]);
  });

  it("or that every member of a Peer network can invite", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Peer", id_hex: "ab12" });
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    expect(card.get("#add-net-kind-preview").text()).toBe(PEER_LINE);
  });

  it("a key without a name has a line without a name", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: null, kind: "Managed", id_hex: "ab12" });
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    expect(card.get("#add-net-kind-preview").text()).toBe("Network · Managed: admins decide who joins");
  });

  it("an emptied field takes the line away", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Managed", id_hex: "ab12" });
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    await card.when.typed("");
    expect(card.exists("#add-net-kind-preview")).toBe(false);
  });

  it("is not shown in create mode", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Managed", id_hex: "ab12" });
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    await card.when.clicked(CREATE_TOGGLE);
    expect(card.exists("#add-net-kind-preview")).toBe(false);
  });

  it("does not change the join: the key alone goes to the slot", async () => {
    const card = await setup({ context });
    answersInspect(card.node, { label: "work", kind: "Managed", id_hex: "ab12" });
    answersJoin(card.node);
    await card.when.typed(INVITE);
    await card.when.pauseOver();
    await card.when.clicked("Add network");
    expect(card.joinBodies()).toEqual([{ key: INVITE }]);
    expect(card.wrapper.emitted("done")).toEqual([["joined", FRESH]]);
  });

  describe("the node refuses to describe it (422)", () => {
    it("there is no line and no error", async () => {
      const card = await setup({ context });
      answersInspect(card.node, { status: 422, body: { error: "invalid_key", message: "Not an invite" } });
      await card.when.typed(INVITE);
      await card.when.pauseOver();
      expect(card.inspectCount()).toBe(1);
      expect(card.exists("#add-net-kind-preview")).toBe(false);
      expect(card.alerts()).toEqual([]);
    });

    it("\"Add network\" still works", async () => {
      const card = await setup({ context });
      answersInspect(card.node, { status: 422, body: { error: "invalid_key", message: "Not an invite" } });
      answersJoin(card.node);
      await card.when.typed(INVITE);
      await card.when.pauseOver();
      await card.when.clicked("Add network");
      expect(card.joinBodies()).toEqual([{ key: INVITE }]);
      expect(card.wrapper.emitted("done")).toEqual([["joined", FRESH]]);
    });
  });
});

async function setup({ context = "peers", opts = withFreshSlot(twoNets()) } = {}) {
  const node = createFakeNode(opts);
  vi.stubGlobal("fetch", node.fetch);
  sessionStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  acceptOverview(store, parseOverview(await api("GET", "/api/overview")));
  const wrapper = mount(AddNetCard, { props: { context }, attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  await settle();
  mounted = { wrapper, node };
  return cardOf(wrapper, node);
}

const compact = (text) => text.replace(/\s+/g, " ").trim();

function cardOf(wrapper, node) {
  const root = () => wrapper.get("#add-net-card");
  const controls = () => root().findAll("button, a").filter((c) => shown(c.element));
  const textFields = () => root().findAll("input[type=text]").filter((i) => shown(i.element));
  const posts = () => node.calls.filter((c) => c.method === "POST");
  const inspects = () => posts().filter((c) => c.path === INSPECT);
  return {
    wrapper, node,
    get: (selector) => root().get(selector),
    exists: (selector) => root().find(selector).exists(),
    text: () => compact(visibleText(root().element)),
    alerts: () => root().findAll("[role=alert]").filter((a) => shown(a.element) && compact(a.text()) !== "").map((a) => compact(a.text())),
    postedPaths: () => posts().map((c) => c.path),
    postedBodies: () => posts().map((c) => c.body),
    inspectCount: () => inspects().length,
    inspectBodies: () => inspects().map((c) => c.body),
    joinBodies: () => posts().filter((c) => c.path.endsWith("/network/join")).map((c) => c.body),
    when: {
      async typed(value) {
        await textFields()[0].setValue(value);
        await settle();
      },
      // Clicks the card (its label), as a person does: the label forwards the click to the radio.
      async kindChosen(id) {
        root().get("#" + id).element.closest("label").click();
        await settle();
      },
      async pauseOver() {
        await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS + 1);
        await settle();
      },
      async clicked(name) {
        const control = controls().find((c) => compact(c.text()) === name);
        expect(control, "a visible control named " + name).toBeDefined();
        await control.trigger("click");
        await settle();
        await settle();
      },
    },
  };
}

// The node holds one empty slot, which `POST /api/nets` hands out.
function withFreshSlot(opts) {
  opts.slots.push({ id: FRESH });
  return opts;
}

const fresh = (node) => node.slotList.find((s) => s.id === FRESH);

// What the node says about a pasted key: an answer or `{ status, body }`.
const answersInspect = (node, answer) => (node.routes["POST " + INSPECT] = answer);

function answersCreate(node) {
  const slot = fresh(node);
  slot.routes["POST /api/network/create"] = () => {
    becomesNetwork(slot, "work", "net-cccc");
    return { key: "mesh1-secret", label: "work", restart: false };
  };
}

function answersJoin(node) {
  const slot = fresh(node);
  slot.routes["POST /api/network/join"] = () => {
    becomesNetwork(slot, "work", "net-cccc");
    return { label: "work", restart: false };
  };
}

function becomesNetwork(slot, label, idHex) {
  slot.joining = false;
  slot.label = label;
  slot.id_hex = idHex;
  slot.state = baseState({ network: { id_hex: idHex, masked: "mesh1-••••-••••" }, peers: [] });
  slot.routes["GET /api/members"] = membersReply({ rows: [selfRow()] });
}
