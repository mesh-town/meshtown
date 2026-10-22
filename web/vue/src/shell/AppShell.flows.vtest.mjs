/* End-to-end shell cases without the binary and the browser (moved from e2e: basic, runtime, shell-conn-strip,
 * shell-bad-token, shell-survives-poll): the whole page on a mock node (`fetch`), the real `api.js`,
 * token, `ApiError` and poller. Visibility is by the `hidden`/`style` attribute: happy-dom does not compute styles. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { getToken } from "../core/token.ts";
import { DEMO_PEER_ONE, DEMO_PEER_TWO, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const NOT_RESPONDING = "Mesh Town is not responding";
const RESPONDS_AGAIN = "Mesh Town is responding again";
const DOWN_KEYWORDS = ["not responding", "no connection", "connection lost", "reconnecting", "unavailable", "offline", "no link", "link lost"];
const STEP_MS = 500;

afterEach(unmountPage);

const makeNode = (over = {}) => createFakeNode({ state: demoState(), config: demoConfig(), ...over });
// A node that shows "stopping…": the last snapshot before it goes silent.
const stoppingNode = () => makeNode({ state: demoState({ node: { ...demoState().node, status: "stopping" } }) });

const textOf = (wrapper, selector) => wrapper.get(selector).text().trim();
const stripText = (wrapper) => textOf(wrapper, "#conn-strip");
const stripShown = (wrapper, text) => shown(wrapper.get("#conn-strip").element) && stripText(wrapper).includes(text);

async function advance(ms) {
  await vi.advanceTimersByTimeAsync(ms);
  await settle();
}

async function advanceUntil(done, limitMs) {
  for (let spent = 0; !done() && spent < limitMs; spent += STEP_MS) await advance(STEP_MS);
}

describe("a regular start", () => {
  it("the first screen is visible, the \"Log\" tab shows its panel", async () => {
    const { wrapper } = await mountPage(makeNode());
    expect(shown(wrapper.get("#sec-home").element)).toBe(true);
    await openTab(wrapper, "log");
    expect(wrapper.get("#tab-log").attributes("aria-selected")).toBe("true");
    expect(shown(wrapper.get("#sec-log").element)).toBe(true);
  });
});

describe("--no-network", () => {
  const noNetwork = () => makeNode({ state: demoState({ network: null, peers: [], domains: [], ports: { published: [], network: [] } }) });
  const headings = (wrapper) => wrapper.findAll("h1,h2,h3,h4").filter((h) => h.text().trim() === "You haven't joined a Mesh Town network yet");
  const button = (wrapper, name) => wrapper.findAll("button").filter((b) => b.text().trim() === name);

  it("the page is alive, the selection panel and status are visible", async () => {
    const { wrapper } = await mountPage(noNetwork());
    expect(wrapper.text().length).toBeGreaterThan(100);
    expect(wrapper.find("#sec-home").exists()).toBe(true);
    expect(headings(wrapper).map((h) => shown(h.element))).toContain(true);
    expect(button(wrapper, "Join with an invite").map((b) => shown(b.element))).toContain(true);
    expect(button(wrapper, "Create a new network").map((b) => shown(b.element))).toContain(true);
    const status = wrapper.get("#status-line");
    expect(shown(status.element)).toBe(true);
    expect(status.text()).toMatch(/^\s*You haven't joined a Mesh Town network yet\s*$/);
    await openTab(wrapper, "peers");
    expect(wrapper.get("#sec-peers").text()).toContain("You haven't joined a Mesh Town network yet");
  });
});

describe("the node does not respond", () => {
  it("the page does not crash, the unavailability indicator is visible", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);
    const before = wrapper.text().length;
    node.fail = "down";
    await advance(10000);
    const after = wrapper.text().length;
    const homeExists = wrapper.find("#sec-home").exists();
    expect(after > 100 && (homeExists || before > 100), `body_len before=${before}, after=${after}`).toBe(true);
    const lower = wrapper.text().toLowerCase();
    expect(DOWN_KEYWORDS.some((k) => lower.includes(k))).toBe(true);
  });
});

describe("polling breaks and the connection returns", () => {
  it("\"not responding\" on the strip and in the line; the connection returned — \"is responding again\", after ~4 s the strip is gone", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);
    node.fail = "down";
    await advanceUntil(() => stripShown(wrapper, NOT_RESPONDING), 15000);
    expect(stripShown(wrapper, NOT_RESPONDING)).toBe(true);
    expect(textOf(wrapper, "#status-line")).toBe(NOT_RESPONDING);

    node.fail = null;
    await advanceUntil(() => stripShown(wrapper, RESPONDS_AGAIN), 25000);
    expect(stripShown(wrapper, RESPONDS_AGAIN)).toBe(true);
    expect(stripText(wrapper)).not.toContain(NOT_RESPONDING);

    await advanceUntil(() => !stripShown(wrapper, RESPONDS_AGAIN), 8000);
    expect(stripShown(wrapper, RESPONDS_AGAIN)).toBe(false);
    expect(textOf(wrapper, "#status-line")).not.toBe(NOT_RESPONDING);
  });
});

describe("the \"Home\" panel and header with a silent node: the node died while showing \"stopping…\"", () => {
  const STOPPING = "Mesh Town is stopping…";
  const STOPPED = "Mesh Town is stopped";
  const statusText = (wrapper) => textOf(wrapper, "#status-line");
  const panelText = (wrapper) => textOf(wrapper, "#route-state");
  const both = (wrapper, text) => statusText(wrapper) === text && panelText(wrapper) === text;

  it("before the break the header and panel speak by the snapshot: \"Mesh Town is stopping…\"", async () => {
    const { wrapper } = await mountPage(stoppingNode());

    expect(statusText(wrapper)).toBe(STOPPING);
    expect(panelText(wrapper)).toBe(STOPPING);
  });

  it("two polls in a row failed — #status-line and #route-state are both \"Mesh Town is not responding\"", async () => {
    const node = stoppingNode();
    const { wrapper } = await mountPage(node);

    node.fail = "down";
    await advanceUntil(() => both(wrapper, NOT_RESPONDING), 15000);

    expect(statusText(wrapper)).toBe(NOT_RESPONDING);
    expect(panelText(wrapper)).toBe(NOT_RESPONDING);
  });

  it("the node came back — the panel is by the snapshot again, like the header: \"Mesh Town is stopping…\"", async () => {
    const node = stoppingNode();
    const { wrapper } = await mountPage(node);
    node.fail = "down";
    await advanceUntil(() => both(wrapper, NOT_RESPONDING), 15000);
    expect(panelText(wrapper), "given: while the node is silent, the panel says \"not responding\"").toBe(NOT_RESPONDING);

    node.fail = null;
    await advanceUntil(() => both(wrapper, STOPPING), 25000);

    expect(panelText(wrapper)).toBe(STOPPING);
    expect(statusText(wrapper)).toBe(STOPPING);
  });

  it("the node answered 503 — #status-line and #route-state are both \"Mesh Town is stopped\"", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);

    node.fail = { path: "/api/state", status: 503, body: { error: "stopping" } };
    await advanceUntil(() => both(wrapper, STOPPED), 20000);

    expect(statusText(wrapper)).toBe(STOPPED);
    expect(panelText(wrapper)).toBe(STOPPED);
  });

  it("the node answered 401 — the sign-in screen instead of the header and panel (no \"not responding\" words)", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);

    node.fail = { path: "/api/state", status: 401, body: { error: "bad_token" } };
    await advanceUntil(() => wrapper.find("#login-form").exists(), 15000);

    expect(wrapper.find("#status-line").exists()).toBe(false);
    expect(wrapper.find("#route-state").exists()).toBe(false);
    expect(wrapper.text()).not.toContain(NOT_RESPONDING);
  });
});

describe("the header and \"Home\" say the same about how many members are in the network", () => {
  const self = () => demoState().peers.filter((p) => p.self);
  const peerOneOffline = () => [...self(), { ...DEMO_PEER_ONE, online: false }];

  // A demo snapshot without the `transport` block: the address directory is not connected — this is the "loading network data" phase.
  it("the snapshot has only its own row, no directory — the header ends with \"loading network data…\", the \"Home\" counter stays \"Only you in …\"", async () => {
    const { wrapper } = await mountPage(makeNode({ state: demoState({ peers: self() }) }));

    expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
    expect(textOf(wrapper, "#home-members-count")).toContain("Only you in");
  });

  it("its own row and one offline member, no directory — the header \"loading network data…\", the counter \"you and 1 other, none online\"", async () => {
    const { wrapper } = await mountPage(makeNode({ state: demoState({ peers: peerOneOffline() }) }));

    expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
    expect(textOf(wrapper, "#home-members-count")).toContain("you and 1 other, none online");
  });

  it("an offline member does not turn \"just me\" into \"no others online\" either in the header or on \"Home\"", async () => {
    const { wrapper } = await mountPage(makeNode({ state: demoState({ peers: peerOneOffline() }) }));

    expect(textOf(wrapper, "#status-line")).not.toContain("only you for now");
    expect(textOf(wrapper, "#home-members-count")).not.toContain("Only you");
  });
});

// In one snapshot the header (`#status-line`) and "Home" (`#home-net-status`) name the same network state.
describe("three network states: the header and \"Home\" say the same", () => {
  describe("only you in the network, the address directory is connected", () => {
    it("the header — \"invite members\", the \"Home\" counter invites, the status line is hidden", async () => {
      const { wrapper } = await setup({ peers: selfOnly(), transport: dht(true) });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · invite members$/);
      expect(textOf(wrapper, "#home-members-count")).toContain("Invite the people you trust");
      expect(textOf(wrapper, "#home-net-status")).toBe("");
    });
  });

  describe("the address directory is not connected", () => {
    it("only its own row — the header \"loading network data…\", \"Home\" has no status", async () => {
      const { wrapper } = await setup({ peers: selfOnly(), transport: dht(false) });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
      expect(textOf(wrapper, "#home-net-status")).toBe("");
    });

    it("offline members are known — the same: loading overrides \"offline\"", async () => {
      const { wrapper } = await setup({ peers: selfAndOffline(2), transport: dht(false) });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
      expect(textOf(wrapper, "#home-net-status")).toBe("Loading network data…");
    });
  });

  describe("others are known, all offline, the address directory is connected", () => {
    it("two — the header \"all other members are offline\", \"Home\" \"All other members are offline…\"", async () => {
      const { wrapper } = await setup({ peers: selfAndOffline(2), transport: dht(true) });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · all other members are offline$/);
      expect(textOf(wrapper, "#home-net-status")).toMatch(/^All other members are offline/);
    });

    it("one — the header \"the other member is offline\", \"Home\" \"The other member is offline…\"", async () => {
      const { wrapper } = await setup({ peers: selfAndOffline(1), transport: dht(true) });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · the other member is offline$/);
      expect(textOf(wrapper, "#home-net-status")).toMatch(/^The other member is offline/);
    });
  });

  describe("others are online", () => {
    for (const joined of [true, false]) {
      it(`the header "2 members online", the status on "Home" is hidden (directory joined: ${joined})`, async () => {
        const { wrapper } = await setup({ peers: demoState().peers, transport: dht(joined) });

        expect(textOf(wrapper, "#status-line")).toMatch(/ · 2 members online$/);
        expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
      });
    }
  });
});

// A URL without token= is made here by the `mountPage` helper (`adoptUrlToken`); the wiring of `main.ts` itself is held by `main.vtest.mjs`.
describe("401 on polling", () => {
  it("the sign-in screen instead of the strip; password sign-in resumes polling; the URL has no token=", async () => {
    const node = makeNode({ routes: { "POST /api/auth/login": { status: 200, body: { via: "password" } } } });
    const { wrapper } = await mountPage(node, { urlToken: "old-token" });
    expect(location.href).not.toContain("token=");

    node.fail = { path: "/api/state", status: 401, body: { error: "bad_token" } };
    await advanceUntil(() => wrapper.find("#login-form").exists(), 15000);
    expect(wrapper.find("#login-form").exists()).toBe(true);
    expect(wrapper.find("#token-url-input").exists()).toBe(false);
    expect(wrapper.find("#status-line").exists()).toBe(false);

    node.fail = null;
    const ok = node.count("GET", "/api/state");
    await wrapper.get("#login-password").setValue("correct-password");
    await wrapper.get("#login-form").trigger("submit");
    await polls(node, 1);
    expect(node.count("GET", "/api/state")).toBeGreaterThan(ok);
    expect(wrapper.find("#login-form").exists()).toBe(false);
    expect(textOf(wrapper, "#status-line")).not.toBe(NOT_RESPONDING);
  });
});

describe("nodes survive polling", () => {
  it("the status line, the \"Settings\" tab and the connection strip — the same nodes after two /api/state responses", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node);
    const strip = wrapper.get("#conn-strip").element;
    const status = wrapper.get("#status-line").element;
    const settingsTab = wrapper.get("#tab-settings").element;

    await polls(node, 2);

    expect(status.isConnected, "the status line node was recreated by polling").toBe(true);
    expect(settingsTab.isConnected, "the \"Settings\" tab node was recreated by polling").toBe(true);
    expect(strip.isConnected, "the connection strip node was recreated by polling").toBe(true);
    expect(wrapper.get("#status-line").element).toBe(status);
    expect(wrapper.get("#tab-settings").element).toBe(settingsTab);
    expect(wrapper.get("#conn-strip").element).toBe(strip);
  });
});

// Helpers for the "three network states": a snapshot with the needed member rows and an address directory block.
const dht = (joined, nodes = 0) => ({ dht: { joined, nodes, last_publish_sec: null } });
const selfOnly = () => demoState().peers.filter((p) => p.self);

// Its own row and `n` known others, none of them online (from two demo members).
const selfAndOffline = (n) => [...selfOnly(), ...[DEMO_PEER_ONE, DEMO_PEER_TWO].slice(0, n).map((p) => ({ ...p, online: false }))];

const setup = ({ peers, transport }) => mountPage(makeNode({ state: demoState({ peers, transport }) }));
