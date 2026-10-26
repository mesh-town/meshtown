/* A store filled as the overview does it, for several networks. A spec: `{ id, label, offers, exits, proxy }`;
 * an offer is a full member id (offers an exit) or `{ full, exit }`. */

import { storeSlotConfig } from "../core/config-meta.ts";
import { acceptOverview } from "../core/overview.ts";
import { createStore } from "../core/store.ts";
import { baseConfig, baseState } from "./section.mjs";

const fullOf = (o) => (typeof o === "string" ? o : o.full);
const offersExit = (o) => (typeof o === "string" ? true : o.exit);

const peerOf = (o) => {
  const full = fullOf(o);
  return { id: full.slice(0, 8).toLowerCase(), id_full: full, name: "exit-" + full[0].toLowerCase(), country: null, version: null, online: true, path: "direct", rtt_ms: 20, exit: offersExit(o) };
};
const chosenOf = (full) => ({ id: full.slice(0, 8).toLowerCase(), id_full: full, name: "exit-" + full[0].toLowerCase(), country: null, exit: true, online: true, active: true });

function stateOfSpec(spec) {
  const me = baseState().peers.find((p) => p.self);
  return baseState({
    network: { id_hex: "hex-" + spec.id, label: spec.label, masked: "mesh2-••••-••••" },
    peers: [me, ...spec.offers.map(peerOf)],
    route: { state: spec.exits.length ? "exit" : "direct", exits: spec.exits.map(chosenOf) },
    config_rev: 1,
  });
}

function briefOf(spec, state) {
  return {
    id: spec.id, label: spec.label, id_hex: "hex-" + spec.id, status: "online", exit_enabled: false, proxy: spec.proxy,
    route_state: state.route.state, exits: spec.exits, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: spec.offers.length, peers_total: spec.offers.length,
  };
}

export function netsStore(specs) {
  const store = createStore();
  const states = specs.map(stateOfSpec);
  acceptOverview(store, {
    api: 2,
    nets: specs.map((s, i) => briefOf(s, states[i])),
    slots: specs.map((s, i) => ({ slot: s.id, state: states[i], members: null })),
    restart_slots: [],
  });
  specs.forEach((s) => {
    const config = baseConfig({ route: { exits: s.exits } });
    if (s.id === "main") Object.assign(store, { config, configRev: 1 });
    else storeSlotConfig(store, s.id, { rev: 1, config }, "boot-1");
  });
  return store;
}
