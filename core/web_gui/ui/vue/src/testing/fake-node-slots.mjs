/* Network slots of the fake node (part of `fake-node.mjs`): `GET /api/overview`, `GET`/`POST /api/nets`, the address
 * `/api/net/<slot>/…` and `members/leave`, per `docs/WEB-API.md` (1.11). Without the `slots` option the node has one slot,
 * `main`, which is the node itself: the overview is built from `node.state` and the member list route, so a test written for one
 * network keeps working. `legacy: true` makes the overview and `/api/nets` answer 404 `not_found`, as a node of an older version.
 *
 * A slot spec (`slots: [...]`): `{ id, label?, id_hex?, members?, state?, config?, rev? }`. `main` uses the top-level `state` and
 * `config` of the node; another slot has its own. `members` is the reply of the slot's member list (`null` — no network).
 * A slot with a key has `id_hex != null`; `label` may stay `null` (a key without a label). */

const MAX_NETS = 8;
const MAIN = "main";
const JSON_HEADERS = { "Content-Type": "application/json" };

const reply = (status, body) => new Response(JSON.stringify(body), { status, headers: JSON_HEADERS });
const copy = (value) => structuredClone(value);
const bodyOf = (res) => res.json();

function emptyState(node) {
  return { ...copy(node.state), network: null, peers: [], domains: [], ports: { published: [], network: [] }, members: null };
}

function makeSlot(node, spec) {
  const own = spec.id === MAIN ? node : { state: spec.state ?? emptyState(node), config: spec.config ?? copy(node.config), rev: spec.rev ?? 1, log: [], routes: {}, pending: [], running: null, fail: null, unexpected: node.unexpected };
  if (spec.id === MAIN && spec.members && !("GET /api/members" in node.routes)) node.routes["GET /api/members"] = spec.members;
  if (spec.id !== MAIN && spec.members) own.routes["GET /api/members"] = spec.members;
  const network = own.state.network ?? null;
  const pick = (key) => (spec[key] !== undefined ? spec[key] : network?.[key] ?? null);
  Object.assign(own, { id: spec.id, label: pick("label"), id_hex: pick("id_hex"), members: spec.members ?? null });
  return own;
}

/** The slot list of a node: the `slots` option, or the node itself as `main`. */
export function makeSlots(node, specs) {
  const list = specs && specs.length > 0 ? specs : [{ id: MAIN }];
  node.slotList = list.map((spec) => makeSlot(node, spec));
  node.slotsGiven = Boolean(specs);
  node.slotSeq = 0;
}

function brief(slot) {
  const state = slot.state;
  const peers = (state.peers || []).filter((p) => !p.self);
  return {
    id: slot.id, label: slot.label, id_hex: slot.id_hex, status: state.node?.status ?? "running", exit_enabled: Boolean(state.exit?.enabled),
    proxy: slot.id === MAIN, route_state: state.route?.state ?? "direct", exits: (state.route?.exits || []).map((e) => e.id_full),
    members_voting: state.members?.voting ?? 0, joining: slot.joining ?? false, members_admitting: state.members?.admitting ?? 0,
    me_excluded: state.members?.me_excluded ?? false, me_pending: state.members?.me_pending ?? false,
    peers_online: peers.filter((p) => p.online).length, peers_total: peers.length,
  };
}

const briefs = (node) => node.slotList.map(brief);
const notFound = () => reply(404, { error: "not_found" });
const unknownNet = () => reply(404, { error: "unknown_net" });

async function slotEntry(node, slot, ops) {
  const state = await ops.dispatch(slot, "GET", new URL("http://node.test/api/state"));
  if (state.status !== 200) return state;
  const members = await ops.dispatch(slot, "GET", new URL("http://node.test/api/members"));
  const restart = (slot.pending || []).length > 0 || (slot.state.restart_pending || []).length > 0;
  return { entry: { slot: slot.id, state: await bodyOf(state), members: members.status === 200 ? await bodyOf(members) : null }, restart };
}

/** `GET /api/overview`: every slot's state and members as its own routes answer them (so route overrides and `fail` act on it). */
export async function overviewReply(node, ops) {
  if (node.legacy) return notFound();
  const slots = [];
  const restart = [];
  for (const slot of node.slotList) {
    const failed = ops.failure(slot === node ? node.fail : null, "/api/state");
    if (failed) return failed;
    const got = await slotEntry(node, slot, ops);
    if (got instanceof Response) return got;
    slots.push(got.entry);
    if (got.restart) restart.push(slot.id);
  }
  return reply(200, { api: 2, nets: briefs(node), slots, restart_slots: restart });
}

export const netsReply = (node) => (node.legacy ? notFound() : reply(200, { api: 2, nets: briefs(node) }));

/** `POST /api/nets`: hands back an empty slot that already waits, else adds one. */
export function addNet(node) {
  if (node.legacy) return notFound();
  const waiting = node.slotList.find((s) => s.id_hex === null && s.id !== MAIN);
  if (waiting) return reply(200, { slot: waiting.id, fresh: false, nets: briefs(node) });
  if (node.slotList.length >= MAX_NETS) return reply(409, { error: "too_many_nets", limit: MAX_NETS });
  node.slotSeq += 1;
  const slot = makeSlot(node, { id: "net" + "abcdefgh"[(node.slotSeq - 1) % 8] + "2" + (2 + ((node.slotSeq - 1) % 6)), state: emptyState(node) });
  node.slotList.push(slot);
  return reply(201, { slot: slot.id, fresh: true, nets: briefs(node) });
}

function leaveReply(node, slot) {
  const members = slot.members ?? { members: [], counts: { members: 0, voting: 0, excluded: 0, unlisted: 0, pending: 0 } };
  if (slot.id !== MAIN) node.slotList = node.slotList.filter((s) => s !== slot);
  return reply(200, { left: true, announced: false, nets: briefs(node), ...copy(members) });
}

/** A request to a slot's address: `rest` is the path after `/api/net/<id>`; `null` — not a slot request. */
export async function slotRequest(node, method, url, body, ops) {
  const m = /^\/api\/net\/([^/]+)(\/.*)$/.exec(url.pathname);
  if (!m) return null;
  const slot = node.slotList.find((s) => s.id === m[1]);
  if (!slot || !node.slotsGiven) return unknownNet();
  if (m[2] === "/log") return notFound();
  if (method === "POST" && m[2] === "/members/leave") return leaveReply(node, slot);
  return ops.dispatch(slot, method, new URL("/api" + m[2] + url.search, "http://node.test"), body);
}

/** `POST /api/members/leave` of the primary network, when the node has slots. */
export const primaryLeave = (node) => leaveReply(node, node);
