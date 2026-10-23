/* Fixtures shared by the `overview.*.test.mjs` files: briefs, states and overviews of a few networks. */

import { acceptOverview } from "./overview.ts";
import { createStore } from "./store.ts";
import { baseState } from "../testing/section.mjs";
import { membersReply } from "../testing/members-fixtures.mjs";

export const MAIN = "main";
export const B = "k7q2xa";
export const C = "m3n4pq";
export const KEYLESS = { label: null, id_hex: null };

export const plain = (v) => JSON.parse(JSON.stringify(v));
export const tick = () => new Promise((r) => setTimeout(r, 0));

export function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

// A brief of a network with a key and the same look as in the node: B is "my devices", main is "friends".
export function brief(id, over = {}) {
  return {
    id, label: id === MAIN ? "friends" : "my devices", id_hex: id === MAIN ? "3f2a01" : "9c0100", status: "online", exit_enabled: false,
    proxy: id === MAIN, route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 1, peers_total: 2, ...over,
  };
}

// The state of a slot: unique `node`, `socks` and `web` per slot, so that a mix-up of slots shows.
export function stateOf(id, over = {}) {
  const base = baseState();
  const port = id === MAIN ? 1080 : 2080;
  return {
    ...base,
    boot_id: "boot-" + id,
    config_rev: 1,
    node: { ...base.node, id: "node-" + id, status: "online" },
    socks: { addr: "127.0.0.1:" + port },
    web: { access: "token", filter: id },
    log: { ...base.log, ring_lines: id === MAIN ? 2000 : 3000 },
    route: { state: "direct", exits: [] },
    ...over,
  };
}

// An `Overview` as `parseOverview` returns it; a spec is `{ id, brief?, state?: fields | null, members? }`.
export function overviewOf(specs, { restart = [] } = {}) {
  return {
    api: 2,
    nets: specs.map((s) => brief(s.id, s.brief)),
    slots: specs.map((s) => ({
      slot: s.id,
      state: s.state === null ? null : stateOf(s.id, s.state),
      members: "members" in s ? s.members : membersReply(),
    })),
    restart_slots: restart,
  };
}

// The JSON of `GET /api/overview` for main and B; `over` overrides top-level fields.
export function rawOverview(over = {}) {
  return plain({ ...overviewOf([{ id: MAIN }, { id: B }]), ...over });
}

export const twoSpecs = () => [{ id: MAIN }, { id: B }];
export const keylessMainAndB = () => [{ id: MAIN, brief: KEYLESS }, { id: B }];

export function setup(specs, opts) {
  const store = createStore();
  const ov = overviewOf(specs, opts);
  const ids = acceptOverview(store, ov);
  return { store, ov, ids };
}

// What a successful read of the config of a slot leaves in its slice.
export function markConfigRead(store, id) {
  const slice = store.slots[id];
  slice.bootId = slice.state.boot_id;
  slice.configRev = slice.state.config_rev;
  slice.config = {};
}

export const failed = (message) => ({ ok: false, error: { message } });

// A store with main "friends" and B, plus a `write` that records the slots it was called for.
// `fail` — slot → a `{ok:false}` result, `throws` — slot → an error to throw, `gate` — slot → a promise to wait for.
export function setupWrites({ fail = {}, throws = {}, gate = {}, bBrief = {} } = {}) {
  const store = createStore();
  const specs = [{ id: MAIN }, { id: B, brief: bBrief }, { id: C, brief: { label: "work" } }];
  acceptOverview(store, overviewOf(specs));
  const written = [];
  const write = async (slot) => {
    written.push(slot);
    if (gate[slot]) await gate[slot];
    if (throws[slot]) throw throws[slot];
    return fail[slot] ?? { ok: true };
  };
  return { store, written, write };
}
