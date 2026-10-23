/* A fake node for page tests: `node.fetch` answers `/api/state`, `/api/config`, `/api/log`,
 * `POST /api/actions/restart` per the contract in `docs/WEB-API.md` / `docs/WEB-API-config.md`; `GET /api/members` answers 404 (an older node)
 * until a test sets `node.routes["GET /api/members"]`. It is plugged in with
 * `vi.stubGlobal("fetch", node.fetch)`: the real `api.js`, token, `ApiError` and poller work as in the browser.
 * `/api/overview`, `/api/nets` and `/api/net/<slot>/…` — in `fake-node-slots.mjs` (option `slots`; `legacy: true` — a node without them).
 * Writing a config that needs a restart (`socks.ifaces`, `socks.access`) and `web.ifaces` — in `fake-node-config.mjs`. */

import { configMeta, putConfig, restartNode } from "./fake-node-config.mjs";
import { addNet, makeSlots, netsReply, overviewReply, primaryLeave, slotRequest } from "./fake-node-slots.mjs";

const JSON_HEADERS = { "Content-Type": "application/json" };

const reply = (status, body) => new Response(JSON.stringify(body), { status, headers: JSON_HEADERS });
const copy = (value) => structuredClone(value);
const spec = ({ status, body }) => reply(status, body);
const isSpec = (v) => v !== null && typeof v === "object" && typeof v.status === "number" && "body" in v;

function logReply(node, query) {
  const after = Number(query.get("after") || 0);
  const limit = Number(query.get("limit") || 500);
  const lines = node.log.filter((l) => l.seq > after).slice(0, limit);
  const seqs = node.log.map((l) => l.seq);
  return reply(200, { boot_id: node.state.boot_id, first_seq: seqs[0] ?? 0, last_seq: seqs.at(-1) ?? 0, gap: false, lines: copy(lines) });
}

// A node with a password, the page is opened by a header without a session; a test overrides `node.routes["GET /api/auth/status"]`.
const AUTH_STATUS = { password_set: true, access: "localnet", listening: "localnet", via: null };

const BUILTIN = {
  "GET /api/auth/status": () => reply(200, AUTH_STATUS),
  "GET /api/state": (node) => reply(200, { ...copy(node.state), config_rev: node.rev }),
  "GET /api/config": (node) => reply(200, { rev: node.rev, config: copy(node.config), file: null, ...configMeta(node) }),
  "PUT /api/config": (node, body) => spec(putConfig(node, body || {})),
  "POST /api/actions/restart": (node) => spec(restartNode(node)),
  "GET /api/log": (node, _body, query) => logReply(node, query),
  // A node of an older version has no member list: the page asks anyway and must live with the 404.
  // A test that wants the list sets `node.routes["GET /api/members"]` (and the `POST /api/members/<action>` routes).
  "GET /api/members": () => reply(404, { error: "not_found" }),
  "GET /api/overview": (node) => overviewReply(node, OPS),
  "GET /api/nets": (node) => netsReply(node),
  "POST /api/nets": (node) => addNet(node),
  "POST /api/members/leave": (node) => (node.slotsGiven ? primaryLeave(node) : null),
};

function routed(node, key, body) {
  const entry = node.routes[key];
  const value = typeof entry === "function" ? entry(body, node) : entry;
  return isSpec(value) ? reply(value.status, value.body) : reply(200, copy(value));
}

function failure(fail, path) {
  if (fail === "down") throw new TypeError("Failed to fetch");
  if (fail && fail.path === path) return reply(fail.status, fail.body);
  return null;
}

async function respond(node, method, url, body) {
  const key = method + " " + url.pathname;
  if (key in node.routes) return routed(node, key, body);
  const handler = BUILTIN[key];
  const answer = handler ? await handler(node, body, url.searchParams) : null;
  if (answer) return answer;
  const slotted = await slotRequest(node, method, url, body, OPS);
  if (slotted) return slotted;
  node.unexpected.push(key);
  return reply(404, { error: "not_found" });
}

// What the slot module needs from here: answer a request as one slot's node would, and the injected failure.
const OPS = { dispatch: respond, failure };

export function createFakeNode({ state, config, rev = 1, log = [], routes = {}, slots = null, legacy = false }) {
  const node = { state, config, rev, log, routes, calls: [], unexpected: [], fail: null, pending: [], running: null, legacy };
  makeSlots(node, slots);
  node.count = (method, path) => node.calls.filter((c) => c.method === method && c.path === path).length;
  node.fetch = async (input, init = {}) => {
    const url = new URL(String(input), "http://node.test");
    // The node is the only host that answers; an address elsewhere (the release check) is unreachable, as when offline.
    if (url.origin !== "http://node.test") throw new TypeError("Failed to fetch");
    const method = (init.method || "GET").toUpperCase();
    const body = init.body ? JSON.parse(init.body) : undefined;
    node.calls.push({ method, path: url.pathname, body });
    return failure(node.fail, url.pathname) || respond(node, method, url, body);
  };
  return node;
}
