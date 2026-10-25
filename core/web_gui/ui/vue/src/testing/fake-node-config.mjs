/* Config writes and restart of the fake node (part of `fake-node.mjs`): the fields that need a restart
 * (`socks.ifaces`, `socks.access`) and the `web.ifaces` field, which takes effect at once. The contract is the node snapshot shape
 * of the network selection and `docs/WEB-API-config.md`. The functions return `{ status, body }`, `fake-node.mjs` builds the response.
 *
 * The running values (`node.running`) are remembered at the first PUT from the config that lay there before it: a test
 * may adjust `node.config` before mounting, and that is not "pending a restart". Returning a value clears the pending state. */

const RESTART_PATHS = ["socks.ifaces", "socks.access"];
const DEFAULT_PORT = 1080;
const LOOPBACK = "127.0.0.1";

const copy = (value) => structuredClone(value);
const pathGet = (obj, path) => path.split(".").reduce((cur, key) => (cur === null || cur === undefined ? undefined : cur[key]), obj);
const asList = (value) => (Array.isArray(value) ? value : []);
const listOrNull = (value) => (asList(value).length > 0 ? copy(value) : null);

// A field value in a comparable form: an empty list, `null` and "no key" are the same thing ("automatic").
function comparable(node, config, path) {
  const value = pathGet(config, path);
  if (path.endsWith("ifaces")) return asList(value);
  return value ?? node.state.socks?.access ?? null;
}

const differs = (node, a, b, path) => JSON.stringify(comparable(node, a, path)) !== JSON.stringify(comparable(node, b, path));

function rememberRunning(node, config) {
  if (node.running) return;
  node.running = Object.fromEntries(RESTART_PATHS.map((p) => [p, comparable(node, config, p)]));
}

const pendingPaths = (node) => RESTART_PATHS.filter((p) => JSON.stringify(comparable(node, node.config, p)) !== JSON.stringify(node.running[p]));

/** Config metadata for PUT and GET responses: `restart_fields` and `restart_pending` — what is saved but not in effect. */
export function configMeta(node) {
  const pending = node.pending || [];
  return { read_only: [], restart_fields: [...pending], restart_pending: [...pending], cli_overrides: [] };
}

// The page must not lock itself out: a list without the network it is opened through gives 422.
// `["*"]` lets everyone in; an empty list is automatic mode, which closes only the router ISP network.
function closesCurrent(node, list) {
  const via = node.state.node?.via_iface;
  if (!via || list.includes("*")) return false;
  if (list.length > 0) return !list.includes(via);
  const here = (node.state.node.networks || []).find((n) => n.name === via);
  return Boolean(node.state.node.router && here && here.default_route);
}

function invalidWebIfaces(node, config) {
  const value = pathGet(config, "web.ifaces");
  if (value === undefined || value === null || !closesCurrent(node, asList(value))) return null;
  const message = "the list closes the network this page is opened through";
  return { status: 422, body: { error: "invalid", message, fields: [{ path: "web.ifaces", message }] } };
}

function applyWebIfaces(node, config) {
  const value = pathGet(config, "web.ifaces");
  if (value === undefined || !node.state.web) return;
  node.state.web = { ...node.state.web, ifaces: listOrNull(value) };
}

/** `PUT /api/config` of the fake node. */
export function putConfig(node, body) {
  if (body.rev !== node.rev) {
    return { status: 409, body: { error: "stale_rev", message: "settings were changed in another tab or by Mesh Town itself", rev: node.rev, config: copy(node.config) } };
  }
  const invalid = invalidWebIfaces(node, body.config);
  if (invalid) return invalid;
  const before = node.config;
  rememberRunning(node, before);
  node.config = copy(body.config);
  node.rev += 1;
  applyWebIfaces(node, node.config);
  node.pending = pendingPaths(node);
  const changed = RESTART_PATHS.filter((p) => differs(node, before, node.config, p));
  return { status: 200, body: { rev: node.rev, config: copy(node.config), changed, file: null, ...configMeta(node) } };
}

// The network verdict for the saved list: automatic mode closes only the default-route network on a router.
function judge(entry, router, access, list) {
  if (access !== "localnet") return { open: false, by: "none" };
  if (list === null) return { open: entry.present !== false && !(router && entry.default_route), by: "auto" };
  return { open: entry.present !== false && (list.includes("*") || list.includes(entry.name)), by: "list" };
}

const portOf = (socks) => String(socks.listen || "").split(":").pop() || String(DEFAULT_PORT);

// The proxy address in the network the page is opened from: from the machine itself, loopback; from a network, its first address.
function pacUrl(node, networks, socks) {
  const via = node.state.node.via_iface;
  const here = networks.find((n) => n.name === via);
  const host = here && here.open && here.addrs[0] ? here.addrs[0] : LOOPBACK;
  return "http://" + host + ":" + portOf(socks) + "/proxy.pac";
}

/** Recomputes from `node.config` what the node applies at start: `open`/`by`, `socks.lan`/`ifaces`/`access`/`pac_url`.
 * Also works for `{ state, config }` without a node — this is how `nets-states.mjs` builds consistent "snapshot + config" pairs. */
export function syncSocks(node) {
  const socks = node.state.socks;
  if (!socks) return;
  const access = pathGet(node.config, "socks.access") ?? socks.access;
  const list = listOrNull(pathGet(node.config, "socks.ifaces"));
  const router = Boolean(node.state.node.router);
  const networks = (node.state.node.networks || []).map((n) => ({ ...n, ...judge(n, router, access, list) }));
  if (node.state.node.networks) node.state.node.networks = networks;
  const lan = networks.filter((n) => n.open && n.addrs[0]).map((n) => ({ addr: n.addrs[0] + ":" + portOf(socks), iface: n.name }));
  const next = { ...socks, access, ifaces: list, lan };
  node.state.socks = { ...next, pac_url: pacUrl(node, networks, next) };
}

export function syncWeb(node) {
  const web = node.state.web;
  if (!web) return;
  const access = pathGet(node.config, "web.access") ?? web.access;
  node.state.web = { ...web, access, listening: access, ifaces: listOrNull(pathGet(node.config, "web.ifaces")) };
}

/** `POST /api/actions/restart`: what was saved takes effect, a new `boot_id` — the poller rereads the config. */
export function restartNode(node) {
  node.running = null;
  node.pending = [];
  syncSocks(node);
  syncWeb(node);
  node.restarts = (node.restarts || 0) + 1;
  node.bootBase = node.bootBase || node.state.boot_id;
  node.state.boot_id = node.bootBase + "-r" + node.restarts;
  return { status: 202, body: { restart: true } };
}
