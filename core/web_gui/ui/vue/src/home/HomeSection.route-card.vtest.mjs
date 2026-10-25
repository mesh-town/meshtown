/* The "how sites open" card on Home with two networks agrees with the header line: where the header says
 * sites won't open, the card is red too; where the header is calm, the card keeps its usual look. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { STORE_KEY } from "../core/store.ts";
import { netsStore } from "../testing/nets-store.mjs";
import HomeSection from "./HomeSection.vue";
import { routeLine } from "./route-line.ts";

const X = "D".repeat(64);
const Y = "E".repeat(64);
const net = (id, label, offers, exits, proxy) => ({ id, label, offers, exits, proxy });
const WONT = "Sites won't open";
let wrapper;

afterEach(() => wrapper?.unmount());

function show(store) {
  wrapper = mount(HomeSection, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return wrapper;
}

function unavailableIn(store, slot, mutate = () => {}) {
  const state = store.slots[slot].state;
  state.route = { ...state.route, state: "unavailable" };
  mutate(state);
  return store;
}

const card = (w) => w.get("#route-state");
const dotOf = (w) => w.get(".state-panel .state-dot");

describe("Home route card with two networks, proxy in a non-leading one", () => {
  it("exit offline: red \"Sites won't open\" with a red dot", () => {
    const store = unavailableIn(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]), "k7q2xa",
      (s) => s.peers.forEach((p) => { if (p.id_full === Y) p.online = false; }));
    const w = show(store);
    expect(card(w).text()).toBe(WONT);
    expect(dotOf(w).classes()).toContain("red");
  });

  it("exit missing from the peers: red \"Sites won't open\"", () => {
    const store = unavailableIn(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]), "k7q2xa",
      (s) => { s.peers = s.peers.filter((p) => p.id_full !== Y); });
    const w = show(store);
    expect(card(w).text()).toBe(WONT);
    expect(dotOf(w).classes()).toContain("red");
  });

  it("proxy in no network: red \"Sites won't open\"", () => {
    const w = show(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [], false)]));
    expect(card(w).text()).toBe(WONT);
    expect(dotOf(w).classes()).toContain("red");
  });

  it("direct route: the card stays green", () => {
    const w = show(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [], true)]));
    expect(card(w).text()).not.toBe(WONT);
    expect(dotOf(w).classes()).toContain("green");
  });
});

describe("Home route card, proxy in the leading network", () => {
  it("direct route: green \"Sites open directly\"", () => {
    const w = show(netsStore([net("main", "home", [X], [], true), net("k7q2xa", "work", [Y], [], false)]));
    expect(card(w).text()).toContain("Sites open directly");
    expect(dotOf(w).classes()).toContain("green");
  });

  it("unavailable: the leading network's own red panel, not the header text", () => {
    const store = unavailableIn(netsStore([net("main", "home", [X], [X], true), net("k7q2xa", "work", [Y], [], false)]), "main",
      (s) => s.peers.forEach((p) => { if (p.id_full === X) p.online = false; }));
    const w = show(store);
    expect(card(w).text()).not.toBe(WONT);
    expect(dotOf(w).classes()).toContain("red");
  });
});

describe("Home route card in every error state is never the green direct card", () => {
  const notGreen = (w) => {
    expect(dotOf(w).classes()).not.toContain("green");
    expect(card(w).text()).not.toContain("Sites open directly");
  };
  const nets = (proxyMain, proxyOther) => [net("main", "home", [X], proxyMain ? [X] : [], proxyMain), net("k7q2xa", "work", [Y], proxyOther ? [Y] : [], proxyOther)];

  it("exit offline in a non-leading network", () => {
    const store = unavailableIn(netsStore(nets(false, true)), "k7q2xa", (s) => s.peers.forEach((p) => { if (p.id_full === Y) p.online = false; }));
    notGreen(show(store));
  });

  it("exit missing from the peers of a non-leading network", () => {
    const store = unavailableIn(netsStore(nets(false, true)), "k7q2xa", (s) => { s.peers = s.peers.filter((p) => p.id_full !== Y); });
    notGreen(show(store));
  });

  it("proxy in no network", () => {
    notGreen(show(netsStore(nets(false, false))));
  });

  it("leading network unavailable", () => {
    notGreen(show(unavailableIn(netsStore(nets(true, false)), "main")));
  });

  it("the node went silent: the grey absence panel", () => {
    const store = netsStore(nets(true, false));
    store.absence = "silent";
    const w = show(store);
    notGreen(w);
    expect(dotOf(w).classes()).toContain("grey");
  });

  it("no snapshot yet: no route card at all", () => {
    const store = netsStore(nets(true, false));
    store.state = null;
    store.absence = "silent";
    const w = show(store);
    expect(w.find("#route-state").exists()).toBe(false);
  });
});

describe("Home route card with a green \"via\" header", () => {
  const VIA = "Sites open via exit-e (work)";

  it("proxy in a non-leading network, exit online: the card says what the header says, with a green dot", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]);
    expect(routeLine(store)?.text).toBe(VIA);
    const w = show(store);
    expect(card(w).text()).toBe(VIA);
    expect(dotOf(w).classes()).toContain("green");
    expect(card(w).text()).not.toContain("Sites open directly");
  });

  it("the header is null (proxy in the leading network, exit in it): the card keeps its own exit text, as before", () => {
    const store = netsStore([net("main", "home", [X], [X], true), net("k7q2xa", "work", [Y], [], false)]);
    expect(routeLine(store)).toBeNull();
    const w = show(store);
    expect(card(w).text()).toBe("Sites open via exit-d (home)");
    expect(dotOf(w).classes()).toContain("green");
  });

  it("the header is null (proxy in a non-leading network, no exit chosen): still \"Sites open directly\"", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [], true)]);
    expect(routeLine(store)).toBeNull();
    const w = show(store);
    expect(card(w).text()).toBe("Sites open directly");
    expect(dotOf(w).classes()).toContain("green");
  });

  it("one network: no header override, the card never carries a network name in brackets", () => {
    const store = netsStore([net("main", "home", [X], [X], true)]);
    expect(routeLine(store)).toBeNull();
    const w = show(store);
    expect(card(w).text()).toContain("exit-d");
    expect(card(w).text()).not.toMatch(/\(.*\)/);
  });
});
