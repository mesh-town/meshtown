/* Helpers for component tests (`*.vtest.mjs`): mounting a section or card with a real store and a snapshot of the
 * node state. The network is replaced in the test itself via `vi.mock("../core/api.ts", ...)`. */

import { mount } from "@vue/test-utils";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "./snapshot.mjs";

export const SELF_FULL = "A".repeat(64);
export const PEER_FULL = "B".repeat(64);
export const PEER2_FULL = "C".repeat(64);
// Program versions (`node.version`, `peers[].version`): its own and one definitely older — see `baseState`.
export const OWN_VERSION = "0.1.0";
export const OLD_VERSION = "0.0.9";

// A node `state` snapshot: a network, "You" and two members; `over` overrides top-level fields.
// Versions: "You" has its own (`OWN_VERSION`), peer-one an older one (`OLD_VERSION`), the nameless one `null` (an old node).
export function baseState(over = {}) {
  return {
    boot_id: "boot-1",
    node: { id: "aaaa1111", id_full: SELF_FULL, web_addr: "127.0.0.1:7070", version: OWN_VERSION },
    network: { id_hex: "net-aaaa", masked: "mesh1-••••-••••" },
    exit: { enabled: true },
    peers: [
      { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", country: "DE", version: OLD_VERSION, online: true, path: "direct", rtt_ms: 30, exit: true },
      { self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU", version: OWN_VERSION },
      { id: "cccc3333", id_full: PEER2_FULL, name: "", country: null, version: null, online: false, last_seen_sec: 300, exit: false },
    ],
    domains: [],
    ports: { published: [], network: [] },
    log: { ring_lines: 2000, tags: [], tail: [] },
    ...over,
  };
}

export function baseConfig(over = {}) {
  return { route: { exits: [] }, ports: { published: [], opened: [] }, ...over };
}

// `detail` — fields of the store snapshot (`state`, `config`, `configRev`, `configMeta`, `activeSection` …).
export function mountSection(Component, detail = {}) {
  const { props, ...snapshot } = detail;
  const store = createStore();
  applySnapshot(store, { state: baseState(), config: baseConfig(), configRev: 1, ...snapshot });
  const wrapper = mount(Component, { props, attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return { wrapper, store };
}

// An `api` reply from the table "METHOD path" → a value or a function; an unknown request is a test error.
export function apiTable(table) {
  return async (method, path, body) => {
    const entry = table[method + " " + path];
    if (entry === undefined) throw new Error("unexpected request " + method + " " + path);
    return typeof entry === "function" ? entry(body) : entry;
  };
}
