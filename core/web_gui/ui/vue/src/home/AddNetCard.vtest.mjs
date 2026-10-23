/* The "Add another network" form: one component for the Members page (`peers`) and for Settings (`settings`).
 * The card runs on a fake node with two networks (A = `main` "friends", B = `k7q2xa` "my devices") and one empty slot that
 * `POST /api/nets` hands out; a network counts as taken once its slot holds a key (`id_hex`). */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../core/api.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { shown, visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { settle } from "../testing/page.mjs";
import { baseState } from "../testing/section.mjs";
import AddNetCard from "./AddNetCard.vue";

const FRESH = "neta22";
const INVITE = "mesh-invite2-ABCDEFGH";
const STAY_BOTH = 'You stay in "friends" and "my devices". The new network gets its own tab in Members.';
const LIMIT = "Mesh Town can be in up to 8 networks at once. To add this one, leave another network first.";
const JOINING = "Joining the network…";
const MINUTE = "This takes up to a minute. Your other networks keep working.";
const CREATE_TOGGLE = "No invite? Create a new network ▸";
const JOIN_TOGGLE = "Have an invite? Join a network ▸";

let mounted;

beforeEach(() => {
  sessionStorage.clear();
  location.hash = "";
});
afterEach(() => {
  mounted.wrapper.unmount();
  location.hash = "";
  expect(mounted.node.unexpected).toEqual([]);
});

describe("the form", () => {
  describe.each(["peers", "settings"])("in the %s context", (context) => {
    it("is titled and tells which networks you stay in", async () => {
      const card = await setup({ context });
      expect(card.text()).toContain("Add another network");
      expect(card.text()).toContain(STAY_BOTH);
    });

    it("asks for the invite in a field with a paste hint", async () => {
      const card = await setup({ context });
      expect(card.text()).toContain("Invite from a network member");
      expect(card.visibleInput().attributes("placeholder")).toBe("Paste the invite here");
      expect(card.text()).toContain("Paste the entire invite. If a word has a typo, Mesh Town will highlight it.");
    });

    it("has \"Add network\", \"Cancel\" and the way to create a network", async () => {
      const card = await setup({ context });
      expect(card.controlTexts()).toEqual(expect.arrayContaining(["Add network", "Cancel", CREATE_TOGGLE]));
    });

    it("sends nothing until a button is pressed", async () => {
      const card = await setup({ context });
      expect(card.postedPaths()).toEqual([]);
    });
  });

  it("is the same in both contexts", async () => {
    const inPeers = await setup({ context: "peers" });
    const peersText = inPeers.text();
    const peersControls = inPeers.controlTexts();
    mounted.wrapper.unmount();
    const inSettings = await setup({ context: "settings" });
    expect(inSettings.text()).toBe(peersText);
    expect(inSettings.controlTexts()).toEqual(peersControls);
  });

  it("names only the networks of the page: one network, one name", async () => {
    const opts = withFreshSlot(twoNets());
    opts.slots = [opts.slots[0], opts.slots[2]];
    const card = await setup({ opts });
    expect(card.text()).toContain('You stay in "friends". The new network gets its own tab in Members.');
  });
});

describe("Cancel", () => {
  it("in the peers context goes back to the first network's tab", async () => {
    const card = await setup({ context: "peers" });
    await card.when.clicked("Cancel");
    expect(location.hash).toBe("#peers/" + NET_A);
    expect(card.postedPaths()).toEqual([]);
  });

  it("in the settings context empties the form and stays on the page", async () => {
    const card = await setup({ context: "settings" });
    await card.when.typed(INVITE);
    await card.when.clicked("Cancel");
    expect(card.visibleInput().element.value).toBe("");
    expect(location.hash).toBe("");
    expect(card.postedPaths()).toEqual([]);
  });
});

describe("eight networks", () => {
  it("replaces the form with the limit text and asks the node for nothing", async () => {
    const card = await setup({ opts: eightNets() });
    expect(card.text()).toContain(LIMIT);
    expect(card.visibleInputs()).toHaveLength(0);
    expect(card.controlTexts()).not.toContain("Add network");
    expect(card.postedPaths()).toEqual([]);
  });

  it("shows the limit in the settings context too", async () => {
    const card = await setup({ opts: eightNets(), context: "settings" });
    expect(card.text()).toContain(LIMIT);
  });

  it("is not shown at seven", async () => {
    const opts = eightNets();
    opts.slots.splice(7, 1);
    const card = await setup({ opts });
    expect(card.text()).not.toContain(LIMIT);
    expect(card.visibleInputs()).toHaveLength(1);
  });

  it("the node can refuse as well: \"too_many_nets\" turns the form into the limit text", async () => {
    const opts = withFreshSlot(twoNets());
    const card = await setup({ opts });
    card.node.routes["POST /api/nets"] = { status: 409, body: { error: "too_many_nets", limit: 8 } };
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.text()).toContain(LIMIT);
    expect(card.postedPaths()).toEqual(["/api/nets"]);
  });
});

describe("joining by invite", () => {
  it("asks for a slot, then joins on the address of that slot", async () => {
    const card = await setup();
    answersJoin(card.node);
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join"]);
    expect(card.postedBodies()).toEqual([{}, { key: INVITE }]);
  });

  it("tells the page which slot joined", async () => {
    const card = await setup();
    answersJoin(card.node);
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.wrapper.emitted("done")).toEqual([["joined", FRESH]]);
  });

  it("with an empty field says so and sends nothing", async () => {
    const card = await setup();
    await card.when.clicked("Add network");
    expect(card.alerts()).toEqual(["Paste the invite a network member sent you."]);
    expect(card.postedPaths()).toEqual([]);
  });

  it("when the node waits for members, says that it takes up to a minute", async () => {
    const card = await setup();
    answersJoin(card.node, { waiting: true });
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.wrapper.emitted("done")).toEqual([["waiting", FRESH]]);
    expect(card.text()).toContain(JOINING);
    expect(card.text()).toContain(MINUTE);
  });

  it("highlights the word the node did not recognise", async () => {
    const card = await setup();
    const message = 'The word "zebra" isn\'t in the dictionary';
    refusesJoin(card.node, 422, { error: "invalid_key", message, word: "zebra" });
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.marks()).toEqual(['"zebra"']);
    expect(card.alerts()).toEqual([message]);
  });

  it("when the network is already added, says so and offers to open it", async () => {
    const card = await setup();
    refusesJoin(card.node, 409, { error: "net_already_added", slot: NET_B });
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.text()).toContain("You're already in \"my devices\" — this invite isn't needed. Keep it for someone else.");
    expect(card.controlTexts()).toContain('Open "my devices"');
  });

  it("\"Open\" of an added network leads to its tab", async () => {
    const card = await setup();
    refusesJoin(card.node, 409, { error: "net_already_added", slot: NET_B });
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    await card.when.clicked('Open "my devices"');
    expect(location.hash).toBe("#peers/" + NET_B);
  });

  describe("when the node had no empty slot and the join is refused", () => {
    it("the slot taken for it is given back: no empty slot is left on the node", async () => {
      const card = await setup({ opts: twoNets() });
      refusesNewSlotJoin(card.node, 422, { error: "invalid_key", message: "Not an invite" });
      await card.when.typed(INVITE);
      await card.when.clicked("Add network");
      expect(card.alerts()).toEqual(["Not an invite"]);
      expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join", "/api/net/" + FRESH + "/members/leave"]);
      expect(card.node.slotList.map((s) => s.id)).toEqual(["main", NET_B]);
      const overview = await api("GET", "/api/overview");
      expect(overview.nets.filter((n) => n.id_hex === null)).toEqual([]);
    });

    it("an empty slot that was already there is kept", async () => {
      const card = await setup();
      refusesJoin(card.node, 422, { error: "invalid_key", message: "Not an invite" });
      await card.when.typed(INVITE);
      await card.when.clicked("Add network");
      expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join"]);
      expect(card.node.slotList.map((s) => s.id)).toContain(FRESH);
    });
  });
});

describe("creating a network", () => {
  it("the link swaps the invite field for the name field and back", async () => {
    const card = await setup();
    await card.when.clicked(CREATE_TOGGLE);
    expect(card.text()).toContain("Network name (members will see it in the invite)");
    expect(card.controlTexts()).toEqual(expect.arrayContaining(["Create network", "Cancel", JOIN_TOGGLE]));
    expect(card.controlTexts()).not.toContain("Add network");
    expect(card.visibleInputs()).toHaveLength(1);
    await card.when.clicked(JOIN_TOGGLE);
    expect(card.controlTexts()).toContain("Add network");
    expect(card.controlTexts()).not.toContain("Create network");
    expect(card.visibleInput().attributes("placeholder")).toBe("Paste the invite here");
  });

  it("asks for a slot, then creates the network on the address of that slot", async () => {
    const card = await setup();
    fresh(card.node).routes["POST /api/network/create"] = { key: "mesh1-secret", label: "work", restart: false };
    await card.when.clicked(CREATE_TOGGLE);
    await card.when.typed("work");
    await card.when.clicked("Create network");
    expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/create"]);
    expect(card.postedBodies()).toEqual([{}, { label: "work" }]);
    expect(card.wrapper.emitted("done")).toEqual([["created", FRESH]]);
  });

  it("with an empty name says so and sends nothing", async () => {
    const card = await setup();
    await card.when.clicked(CREATE_TOGGLE);
    await card.when.clicked("Create network");
    expect(card.alerts()).toEqual(['Enter a name, for example "family".']);
    expect(card.postedPaths()).toEqual([]);
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
  const inputs = () => root().findAll("input").filter((i) => shown(i.element));
  const posts = () => node.calls.filter((c) => c.method === "POST");
  return {
    wrapper, node,
    text: () => compact(visibleText(root().element)),
    controlTexts: () => controls().map((c) => compact(c.text())),
    visibleInputs: inputs,
    visibleInput() {
      expect(inputs(), "exactly one field is visible").toHaveLength(1);
      return inputs()[0];
    },
    alerts: () => root().findAll("[role=alert]").filter((a) => shown(a.element) && compact(a.text()) !== "").map((a) => compact(a.text())),
    marks: () => root().findAll("mark").filter((m) => shown(m.element)).map((m) => compact(m.text())),
    postedPaths: () => posts().map((c) => c.path),
    postedBodies: () => posts().map((c) => c.body),
    when: {
      async typed(value) {
        await inputs()[0].setValue(value);
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

// Eight networks with a key (A, B and six more) and the empty slot.
function eightNets() {
  const opts = withFreshSlot(twoNets());
  const more = ["nets22", "nets23", "nets24", "nets25", "nets26", "nets27"];
  opts.slots.splice(2, 0, ...more.map((id, i) => ({ id, label: "net " + i, id_hex: "net-" + id, members: membersReply() })));
  return opts;
}

function becomesNetwork(slot, label, idHex) {
  slot.joining = false;
  slot.label = label;
  slot.id_hex = idHex;
  slot.state = baseState({ network: { id_hex: idHex, masked: "mesh1-••••-••••" }, peers: [] });
  slot.routes["GET /api/members"] = membersReply({ rows: [selfRow()] });
}

// The slot answers the join: a network with the label "work", or "waiting" for members.
function answersJoin(node, { waiting = false } = {}) {
  const slot = fresh(node);
  slot.routes["POST /api/network/join"] = () => {
    if (waiting) {
      slot.joining = true;
      return { label: null, restart: false, waiting: true };
    }
    becomesNetwork(slot, "work", "net-cccc");
    return { label: "work", restart: false };
  };
}

// The node has no empty slot, so `POST /api/nets` adds one; its join is refused.
function refusesNewSlotJoin(node, status, body) {
  const fetchOnNode = node.fetch;
  node.fetch = async (input, init) => {
    const res = await fetchOnNode(input, init);
    if (init && init.method === "POST" && String(input).endsWith("/api/nets")) fresh(node).routes["POST /api/network/join"] = { status, body };
    return res;
  };
  vi.stubGlobal("fetch", node.fetch);
}

const refusesJoin = (node, status, body) => (fresh(node).routes["POST /api/network/join"] = { status, body });
