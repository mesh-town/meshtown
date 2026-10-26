/* The "Join or create another network" card with its Join / Create tabs.
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
const INVITE = "mesh1-ABCDEFGH";
const STAY_BOTH = 'You stay in "friends" and "my devices". The new network gets its own tab in Members.';
const LIMIT = "Mesh Town is in 8 networks, the most it can be in. To join a new one, leave a network first. You can still rejoin one of these.";
const LIMIT_REFUSED = "This is a new network, so it can't be added until you leave one.";
const NEW_INVITE = "mesh-invite2-ABCDEFGH";
const JOINING = "Joining the network…";
const MINUTE = "This takes up to a minute. Your other networks keep working.";
const CREATE_TAB = "Create";
const JOIN_TAB = "Join";

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
  describe("the card", () => {
    it("is titled and tells which networks you stay in", async () => {
      const card = await setup();
      expect(card.text()).toContain("Join or create another network");
      expect(card.text()).toContain(STAY_BOTH);
    });

    it("asks for the invite in a field with a paste hint", async () => {
      const card = await setup();
      expect(card.text()).toContain("Invite you received");
      expect(card.visibleInput().attributes("placeholder")).toBe("Paste the invite here");
      expect(card.text()).toContain("Paste the entire invite. If a word has a typo, Mesh Town will highlight it.");
    });

    it("has \"Add network\", \"Cancel\" and the Join and Create tabs", async () => {
      const card = await setup();
      expect(card.controlTexts()).toEqual(expect.arrayContaining(["Add network", "Cancel", JOIN_TAB, CREATE_TAB]));
    });

    it("sends nothing until a button is pressed", async () => {
      const card = await setup();
      expect(card.postedPaths()).toEqual([]);
    });
  });

  it("names only the networks of the page: one network, one name", async () => {
    const opts = withFreshSlot(twoNets());
    opts.slots = [opts.slots[0], opts.slots[2]];
    const card = await setup({ opts });
    expect(card.text()).toContain('You stay in "friends". The new network gets its own tab in Members.');
  });
});

describe("Cancel", () => {
  it("goes back to the first network's tab", async () => {
    const card = await setup();
    await card.when.clicked("Cancel");
    expect(location.hash).toBe("#peers/" + NET_A);
    expect(card.postedPaths()).toEqual([]);
  });

});

describe("a join that waits for members", () => {
  describe("the waiting card", () => {
    it("has the Cancel of the waiting join and no second Cancel of the form", async () => {
      const card = await waiting();
      expect(card.exists("#net-join-cancel"), "Cancel of the join").toBe(true);
      expect(card.exists("#add-net-cancel"), "Cancel of the form").toBe(false);
      expect(card.controlTexts().filter((t) => t === "Cancel")).toHaveLength(1);
    });

    it("Cancel leaves the half-joined slot and sends nothing else", async () => {
      const card = await waiting();
      await card.when.joinCancelled();
      expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join", "/api/net/" + FRESH + "/members/leave"]);
      expect(card.postedBodies()).toEqual([{}, { key: INVITE }, { confirm: true }]);
      expect(card.node.slotList.map((s) => s.id)).not.toContain(FRESH);
    });

    it("after Cancel the form is empty, in join mode, without the waiting line", async () => {
      const card = await waiting();
      await card.when.joinCancelled();
      expect(card.visibleInput().element.value).toBe("");
      expect(card.visibleInput().attributes("placeholder")).toBe("Paste the invite here");
      expect(card.exists("#net-joining")).toBe(false);
      expect(card.text()).not.toContain(JOINING);
      expect(card.controlTexts()).toEqual(expect.arrayContaining(["Add network", "Cancel"]));
      expect(card.wrapper.find("#add-net-card").exists(), "the form stays open").toBe(true);
    });
  });

  it("Cancel keeps the \"add\" tab", async () => {
    const card = await waiting();
    await card.when.joinCancelled();
    expect(card.store.peersTab).toBe("add");
  });
});

describe("eight networks", () => {
  it("keeps the field, says the limit above it and offers no way to create", async () => {
    const card = await setup({ opts: eightNets() });
    expect(card.text()).toContain(LIMIT);
    expect(card.visibleInputs()).toHaveLength(1);
    expect(card.controlTexts()).toContain("Add network");
    expect(card.controlTexts()).not.toContain(CREATE_TAB);
    expect(card.exists("#add-net-tab-create")).toBe(false);
    expect(card.postedPaths()).toEqual([]);
  });

  it("refuses a word key before any request", async () => {
    const card = await setup({ opts: eightNets() });
    await card.when.typed(INVITE);
    await card.when.clicked("Add network");
    expect(card.alerts()).toEqual([LIMIT_REFUSED]);
    expect(card.postedPaths()).toEqual([]);
  });

  it("is not shown at seven", async () => {
    const opts = eightNets();
    opts.slots.splice(7, 1);
    const card = await setup({ opts });
    expect(card.text()).not.toContain(LIMIT);
    expect(card.visibleInputs()).toHaveLength(1);
  });

  it("the node can refuse an invite as well: \"too_many_nets\" gives the refusal text in the field's error", async () => {
    const opts = withFreshSlot(twoNets());
    const card = await setup({ opts });
    card.node.routes["POST /api/network/join"] = { status: 409, body: { error: "too_many_nets", limit: 8 } };
    await card.when.typed(NEW_INVITE);
    await card.when.clicked("Add network");
    expect(card.alerts()).toEqual([LIMIT_REFUSED]);
    expect(card.postedPaths()).toEqual(["/api/network/join"]);
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
    expect(card.alerts()).toEqual(["Paste the invite first."]);
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
  it("the tabs swap the invite field for the name field with the kind choice, and back", async () => {
    const card = await setup();
    await card.when.clicked(CREATE_TAB);
    expect(card.text()).toContain("Network name (members will see it in the invite)");
    expect(card.controlTexts()).toEqual(expect.arrayContaining(["Create network", "Cancel", JOIN_TAB, CREATE_TAB]));
    expect(card.controlTexts()).not.toContain("Add network");
    expect(card.visibleInputs()).toHaveLength(1);
    expect(card.exists("#add-net-label-input")).toBe(true);
    expect(card.exists(".kind-picker"), "the kind choice").toBe(true);
    expect(card.wrapper.get("#add-net-tab-create").attributes("aria-selected")).toBe("true");
    await card.when.clicked(JOIN_TAB);
    expect(card.wrapper.get("#add-net-tab-join").attributes("aria-selected")).toBe("true");
    expect(card.controlTexts()).toContain("Add network");
    expect(card.controlTexts()).not.toContain("Create network");
    expect(card.visibleInput().attributes("placeholder")).toBe("Paste the invite here");
  });

  it("asks for a slot, then creates the network on the address of that slot", async () => {
    const card = await setup();
    fresh(card.node).routes["POST /api/network/create"] = { key: "mesh1-secret", label: "work", restart: false };
    await card.when.clicked(CREATE_TAB);
    await card.when.typed("work");
    await card.when.clicked("Create network");
    expect(card.postedPaths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/create"]);
    expect(card.postedBodies()).toEqual([{}, { label: "work" }]);
    expect(card.wrapper.emitted("done")).toEqual([["created", FRESH]]);
  });

  it("with an empty name says so and sends nothing", async () => {
    const card = await setup();
    await card.when.clicked(CREATE_TAB);
    await card.when.clicked("Create network");
    expect(card.alerts()).toEqual(['Enter a name, for example "family".']);
    expect(card.postedPaths()).toEqual([]);
  });
});

async function setup({ opts = withFreshSlot(twoNets()) } = {}) {
  const node = createFakeNode(opts);
  vi.stubGlobal("fetch", node.fetch);
  sessionStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  acceptOverview(store, parseOverview(await api("GET", "/api/overview")));
  const wrapper = mount(AddNetCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  await settle();
  mounted = { wrapper, node };
  return cardOf(wrapper, node, store);
}

// The form after a join that waits for members.
async function waiting(args) {
  const card = await setup(args);
  answersJoin(card.node, { waiting: true });
  await card.when.typed(INVITE);
  await card.when.clicked("Add network");
  expect(card.exists("#net-joining"), "the waiting card is shown").toBe(true);
  return card;
}

const compact = (text) => text.replace(/\s+/g, " ").trim();

function cardOf(wrapper, node, store) {
  const root = () => wrapper.get("#add-net-card");
  const controls = () => root().findAll("button, a").filter((c) => shown(c.element));
  const inputs = () => root().findAll("input[type=text]").filter((i) => shown(i.element));
  const posts = () => node.calls.filter((c) => c.method === "POST");
  return {
    wrapper, node, store,
    exists: (selector) => root().find(selector).exists(),
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
      async joinCancelled() {
        await root().get("#net-join-cancel").trigger("click");
        await settle();
        await settle();
      },
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
