/* Options of `createFakeNode` for a node with two networks: A = `main` "friends", B = `k7q2xa` "my devices". Both have a key
 * (`id_hex`), a member list, peers, domains, published and network ports, a config with `ports.opened` and log lines with `net`.
 * `twoNets(patchB)` merges `patchB` into the spec of B (`{ label, members, state, config, ... }`); `unlabeledNet()` is a node
 * whose second slot has a key but no label. */

import { membersReply } from "./members-fixtures.mjs";
import { baseConfig, baseState } from "./section.mjs";

export const NET_A = "main";
export const NET_B = "k7q2xa";
const ID_HEX_A = "net-aaaa";
const ID_HEX_B = "net-bbbb";

const domain = (name) => ({ name, checks: {} });
const published = (port) => ({ port, name: "web" + port, shared_with: "all" });

function netState(idHex, over = {}) {
  const base = baseState({ network: { id_hex: idHex, masked: "mesh1-••••-••••" }, domains: [domain("example.org")] });
  return { ...base, ports: { published: [published(8080)], network: [] }, ...over };
}

const netConfig = () => baseConfig({ ports: { published: [{ port: 8080 }], opened: [] } });
const logLine = (seq, net) => ({ seq, ts: 1792400000 + seq, level: "info", tag: "net", msg: "line " + seq, net });

/** Options of a node with two networks. */
export function twoNets(patchB = {}) {
  const b = { id: NET_B, label: "my devices", id_hex: ID_HEX_B, members: membersReply(), state: netState(ID_HEX_B), config: netConfig(), ...patchB };
  return {
    state: netState(ID_HEX_A),
    config: netConfig(),
    log: [logLine(1, NET_A), logLine(2, NET_B)],
    slots: [{ id: NET_A, label: "friends", id_hex: ID_HEX_A, members: membersReply() }, b],
  };
}

/** Two networks where B holds a key but has no label. */
export const unlabeledNet = () => twoNets({ label: null });
