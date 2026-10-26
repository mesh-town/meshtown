/* "Add another network" with the one invite form: an invite goes to `/api/network/join` without a slot and the node picks the place;
 * a word key still takes a slot of its own; at the limit the field stays. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../core/api.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { settle } from "../testing/page.mjs";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";
import AddNetCard from "./AddNetCard.vue";

const FRESH = "neta22";
const INVITE = "mesh-invite2-ABCDEFGH";
const WORD_KEY = "mesh2-ABCDEFGH";
const LIMIT_ABOVE = "Mesh Town is in 8 networks, the most it can be in. To join a new one, leave a network first. You can still rejoin one of these.";
const LIMIT_REFUSED = "This is a new network, so it can't be added until you leave one.";

let mounted;
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  sessionStorage.clear();
  location.hash = "";
});
afterEach(() => {
  vi.useRealTimers();
  mounted.wrapper.unmount();
  location.hash = "";
  expect(mounted.node.unexpected).toEqual([]);
});

describe("an invite", () => {
  it("goes to /api/network/join alone and leads to the tab the node named", async () => {
    const card = await setup();
    card.node.routes["POST /api/network/join"] = { label: "work", restart: false, slot: FRESH };
    await card.type(INVITE);
    await card.click("#add-net-btn");
    expect(card.paths()).toEqual(["/api/network/join"]);
    expect(card.node.calls.find((c) => c.path === "/api/network/join").body).toEqual({ key: INVITE });
    expect(location.hash).toBe("#peers/" + FRESH);
  });

  it("for a network the computer is in says so and offers to open it", async () => {
    const card = await setup();
    card.node.routes["POST /api/network/join"] = { status: 409, body: { error: "net_already_added", slot: NET_B } };
    await card.type(INVITE);
    await card.click("#add-net-btn");
    expect(card.text("#add-net-dup")).toContain('You\'re already in "my devices"');
    await card.click("#add-net-open");
    expect(location.hash).toBe("#peers/" + NET_B);
  });
});

describe("a word key", () => {
  it("takes a slot of its own and joins on its address", async () => {
    const card = await setup();
    fresh(card.node).routes["POST /api/network/join"] = { label: "work", restart: false };
    await card.type(WORD_KEY);
    await card.click("#add-net-btn");
    expect(card.paths()).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join"]);
  });
});

describe("eight networks", () => {
  it("keep the field and say the limit above it; the node's refusal reads as the refusal text", async () => {
    const card = await setup({ opts: eightNets() });
    expect(card.text("#add-net-limit")).toBe(LIMIT_ABOVE);
    expect(card.has("#add-net-input")).toBe(true);
    expect(card.has("#add-net-tab-create")).toBe(false);
    card.node.routes["POST /api/network/join"] = { status: 409, body: { error: "too_many_nets", limit: 8 } };
    await card.type(INVITE);
    await card.click("#add-net-btn");
    expect(card.text("#add-net-error")).toBe(LIMIT_REFUSED);
  });

  it("at the limit the card says it once: no note, the error under the field", async () => {
    const card = await setup({ opts: eightNets() });
    card.node.routes["POST /api/network/inspect"] = { label: "work", id_hex: "bb", place: "new" };
    card.node.routes["POST /api/network/join"] = { status: 409, body: { error: "too_many_nets", limit: 8 } };
    await card.type(INVITE);
    await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
    await settle();
    expect(card.has("#add-net-kind-preview"), "the preview is shown").toBe(true);
    expect(card.has("#add-net-place-note"), "the limit stands above the field already").toBe(false);
    await card.click("#add-net-btn");
    expect(card.text("#add-net-error")).toBe(LIMIT_REFUSED);
    expect(card.has("#add-net-place-note")).toBe(false);
  });

  it("let an invite in: it may lead back to a network the computer is in", async () => {
    const card = await setup({ opts: eightNets() });
    card.node.routes["POST /api/network/join"] = { label: "friends", restart: false, slot: "main" };
    await card.type(INVITE);
    await card.click("#add-net-btn");
    expect(card.paths()).toEqual(["/api/network/join"]);
  });
});

function eightNets() {
  const opts = twoNets();
  opts.slots.push({ id: FRESH });
  const more = ["nets22", "nets23", "nets24", "nets25", "nets26", "nets27"];
  opts.slots.splice(2, 0, ...more.map((id, i) => ({ id, label: "net " + i, id_hex: "net-" + id, members: membersReply() })));
  return opts;
}

const fresh = (node) => node.slotList.find((s) => s.id === FRESH);

async function setup({ opts = withFresh(twoNets()) } = {}) {
  const node = createFakeNode(opts);
  vi.stubGlobal("fetch", node.fetch);
  sessionStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  acceptOverview(store, parseOverview(await api("GET", "/api/overview")));
  const wrapper = mount(AddNetCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  await settle();
  mounted = { wrapper, node };
  const root = () => wrapper.get("#add-net-card");
  return {
    node,
    has: (sel) => root().find(sel).exists(),
    text: (sel) => root().get(sel).text().replace(/\s+/g, " ").trim(),
    paths: () => node.calls.filter((c) => c.method === "POST").map((c) => c.path),
    async type(value) {
      await root().get("#add-net-input").setValue(value);
    },
    async click(sel) {
      await root().get(sel).trigger("click");
      await settle();
      await settle();
    },
  };
}

function withFresh(opts) {
  opts.slots.push({ id: FRESH });
  return opts;
}
