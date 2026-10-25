/* Fixtures of a Managed network for the page tests: one network "work" on a fake node, its snapshot (`network.kind`,
 * `network.me_role`, `network.admins`, `network.can_*`, `peers[].role`), its member list (`kind`, `admins`, `me_admin`,
 * `MemberRow.admin`) and the overview extras the fake node does not know (`nets[].kind`, `nets[].me_admin`, `nets[].can_invite`,
 * `notices[]`, `POST /api/notices/ack`).
 *
 * `managedNode({ meRole, admins, canInvite, network, notices, brief })` is a node with the single network "work" where this
 * computer ("finch-unwary") has the role `meRole`; `admins` lists the full ids of the admins (default: this computer and "anna").
 * `served` on the returned node holds the notices the node still keeps (an ack removes one). */

import { createFakeNode } from "./fake-node.mjs";
import { memberRow, membersReply } from "./members-fixtures.mjs";
import { OWN_VERSION, SELF_FULL, baseConfig, baseState } from "./section.mjs";

export const NET_NAME = "work";
export const ID_HEX = "net-aaaa";
export const ME_FULL = SELF_FULL;
export const ANNA_FULL = "b1b1b1b1" + "0".repeat(56);
export const BOB_FULL = "c2c2c2c2" + "0".repeat(56);
const NONAME_FULL = "d3d3d3d3" + "0".repeat(56);

const PEERS = [
  { id_full: ANNA_FULL, id: "b1b1b1b1", name: "anna" },
  { id_full: BOB_FULL, id: "c2c2c2c2", name: "bob" },
  { id_full: NONAME_FULL, id: "d3d3d3d3", name: "" },
];

/** A notice about a deleted network, as the node lists it. */
export const deletedNotice = (over = {}) => ({
  id: 1, kind: "net_deleted", label: "family", id_hex: "dead0001", by: ANNA_FULL, by_name: "anna", at_unix: 1792400000, ...over,
});

const roleOf = (admins, full) => (admins.includes(full) ? "admin" : "member");

/** The snapshot of the Managed network; `over` overrides fields of `network`. */
export function managedState({ meRole = "admin", admins = [ME_FULL, ANNA_FULL], canInvite = meRole === "admin", network = {} } = {}) {
  const isAdmin = meRole === "admin";
  const peers = [
    { self: true, online: true, id: "aaaa1111", id_full: ME_FULL, name: "finch-unwary", country: "RU", version: OWN_VERSION, role: meRole },
    ...PEERS.map((p) => ({ ...p, country: "DE", version: OWN_VERSION, online: true, path: "direct", rtt_ms: 30, exit: false, role: roleOf(admins, p.id_full) })),
  ];
  return baseState({
    network: {
      id_hex: ID_HEX, masked: "mesh1-••••-••••", kind: "Managed", me_role: meRole, admins, root: admins[0],
      can_invite: canInvite, can_remove: isAdmin, can_set_admins: isAdmin, can_delete_network: isAdmin, can_leave: true, ...network,
    },
    peers,
  });
}

/** The member list that goes with `managedState`. */
export function managedMembers(state) {
  const net = state.network;
  const rows = state.peers.map((p) =>
    memberRow({ id: p.id, id_full: p.id_full, name: p.name || null, self: Boolean(p.self), online: p.online, admin: p.role === "admin" }));
  return membersReply({ rows, kind: "Managed", admins: net.admins, me_admin: net.me_role === "admin" });
}

export const nodeConfig = () => baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } });

/** Makes the overview of `node` carry the Managed extras and the notices; returns the live set of notices. */
export function serveManaged(node, { brief = {}, briefs = {}, notices = [] } = {}) {
  const live = { notices: structuredClone(notices), acked: [] };
  const inner = node.fetch;
  node.fetch = async (input, init) => {
    const response = await inner(input, init);
    if (new URL(String(input), "http://node.test").pathname !== "/api/overview" || !response.ok) return response;
    const raw = await response.json();
    raw.nets = raw.nets.map((n) => ({ ...n, ...brief, ...(briefs[n.id] ?? {}) }));
    raw.notices = structuredClone(live.notices);
    return new Response(JSON.stringify(raw), { status: 200, headers: { "Content-Type": "application/json" } });
  };
  node.routes["POST /api/notices/ack"] = (body) => {
    live.acked.push(body);
    live.notices = live.notices.filter((n) => n.id !== body.id);
    return {};
  };
  return live;
}

/** A node with the single Managed network "work" (slot `main`). */
export function managedNode({ notices = [], brief = {}, ...stateOpts } = {}) {
  const state = managedState(stateOpts);
  const node = createFakeNode({
    state, config: nodeConfig(), slots: [{ id: "main", label: NET_NAME, id_hex: ID_HEX, members: managedMembers(state) }],
  });
  const meAdmin = state.network.me_role === "admin";
  node.served = serveManaged(node, { brief: { kind: "Managed", me_admin: meAdmin, can_invite: state.network.can_invite, ...brief }, notices });
  return node;
}

/** A node with a plain Peer network (no managed fields anywhere) and optional notices. */
export function peerNode({ notices = [] } = {}) {
  const state = baseState({ network: { id_hex: ID_HEX, masked: "mesh1-••••-••••" } });
  const node = createFakeNode({
    state, config: nodeConfig(), slots: [{ id: "main", label: NET_NAME, id_hex: ID_HEX, members: membersReply() }],
  });
  node.served = serveManaged(node, { notices });
  return node;
}
