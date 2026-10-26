/* DemoNode data without the binary: a snapshot, a config and a log "like the demo node's" for the cases moved over from e2e.
 * The source of the numbers is `core/examples/web_demo/` (`node.rs`, `snapshot.rs`, `ports.rs`, `config.rs`): if you change
 * DemoNode, change this too; they do not sync silently. */

import { PEER2_FULL, PEER_FULL, SELF_FULL, baseConfig, baseState } from "./section.mjs";

export const DEMO_PEER_ONE = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one" };
export const DEMO_PEER_TWO = { id: "cccc3333", id_full: PEER2_FULL, name: "peer-two" };
export const STRANGER_ID = "eeee5555";

const DEMO_PORT = 9000;
const DEMO_PORT_NAME = "demo-web";
const DEMO_UDP_PORT = 5353;
const DEMO_UDP_NAME = "demo-dns";
const LOG_LINES = 10;

const peers = () => [
  { ...DEMO_PEER_ONE, country: "RU", version: "0.0.9", online: true, path: "direct", rtt_ms: 30, exit: true },
  { self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: null, version: "0.1.0", exit: true },
  { ...DEMO_PEER_TWO, country: null, version: null, online: true, path: "relay", rtt_ms: 30, exit: true },
];

const check = (verdict) => ({ verdict, reason: verdict, dns_ms: 20, tcp_ms: 30, tls_ms: 40, http_ms: 200, status: 200, bytes: 1000, stalled: false, age_sec: 60 });

const domains = () => ["example.org", "example.com", "x.com"].map((name) => ({
  name,
  checks: { [DEMO_PEER_ONE.id]: check("ok"), [DEMO_PEER_TWO.id]: check("ok") },
}));

const networkPorts = () => [{
  node: DEMO_PEER_ONE.id_full, node_name: DEMO_PEER_ONE.name, online: true, port: DEMO_PORT, name: DEMO_PORT_NAME,
  published: true, opened: { local: DEMO_PORT, state: "listening", error: null, last_error: null },
  web_url: "http://" + DEMO_PEER_ONE.name + ".meshtown:" + DEMO_PORT, proto: "tcp",
}, {
  node: DEMO_PEER_ONE.id_full, node_name: DEMO_PEER_ONE.name, online: true, port: DEMO_UDP_PORT, proto: "udp", name: DEMO_UDP_NAME,
  published: true, opened: null, web_url: null,
}];

export function demoLog(n = LOG_LINES) {
  return Array.from({ length: n }, (_, i) => ({ seq: i + 1, t: i * 0.5, tag: "node", text: "start: " + (i + 1) + " ready" }));
}

/** A demo node snapshot in a network: its own node `finch-unwary`, peer-one, peer-two, three domains, port 9000 "demo-web" (TCP) and 5353 "demo-dns" (UDP). */
export function demoState(over = {}) {
  const tail = demoLog().slice(-2);
  const base = baseState();
  return baseState({
    node: { ...base.node, status: "online", identity: "persistent", identity_reason: null },
    network: { label: "demo-network", id_hex: "net-demo", masked: "demo-network--••••••••" },
    peers: peers(),
    domains: domains(),
    ports: { published: [], network: networkPorts() },
    log: { ring_lines: 2000, tags: ["node"], first_seq: 1, last_seq: LOG_LINES, tail },
    ...over,
  });
}

export function demoConfig(over = {}) {
  return baseConfig({ socks: { enabled: true, port: 1080, access: "localhost" }, ...over });
}
