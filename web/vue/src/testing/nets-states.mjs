/* Node snapshots for the network selection tests (network selection): a router, a router with a guest network, a laptop and a
 * laptop with a VPN. The shape: `node.router`, `node.via_iface`, `node.networks[]`, `socks.ifaces`,
 * `socks.lan`, `socks.pac_url`, `web.ifaces`. Each snapshot is on top of `demoState()`.
 *
 * Factory parameters: `access` — `socks.access` ("localnet" by default, "localhost" for the laptop), `ifaces` — the saved
 * list (`null` — automatic), `webAccess` — `web.access`; the rest are top-level fields that override the snapshot.
 * `netsConfig(state)` returns a consistent config: `createFakeNode({ state, config: netsConfig(state) })`. */

import { demoConfig, demoState } from "./demo-node.mjs";
import { syncSocks, syncWeb } from "./fake-node-config.mjs";

const PORT = 1080;
const HOME_IP = "192.168.88.1";
const PROVIDER_IP = "192.168.1.100";
const GUEST_IP = "192.168.9.1";
const LAPTOP_IP = "192.168.1.23";
const VPN_IP = "10.8.0.2";

const entry = (name, addr, extra = {}) => ({ name, addrs: [addr], default_route: false, present: true, open: false, by: "auto", ...extra });
const BR_LAN = () => entry("br-lan", HOME_IP);
const ETH0 = () => entry("eth0", PROVIDER_IP, { default_route: true });
const BR_GUEST = () => entry("br-guest", GUEST_IP);
const EN0 = () => entry("en0", LAPTOP_IP, { default_route: true });
const UTUN3 = () => entry("utun3", VPN_IP, { kind: "tunnel" });

function build({ router, via, networks, access, ifaces, webAccess, over }) {
  const state = demoState({
    node: { ...demoState().node, router, via_iface: via, networks_side: "socks", networks },
    socks: { enabled: true, listen: "127.0.0.1:" + PORT, access, ifaces, lan: [], pac_url: "http://127.0.0.1:" + PORT + "/proxy.pac" },
    web: { access: webAccess, listening: webAccess, ifaces },
    ...over,
  });
  const config = netsConfig(state);
  syncSocks({ state, config });
  syncWeb({ state, config });
  return state;
}

function split(opts, defaults) {
  const { access = defaults.access, ifaces = defaults.ifaces ?? null, webAccess = defaults.webAccess, ...over } = opts;
  return { access, ifaces, webAccess, over };
}

/** Router: the page is opened from `br-lan` (`via_iface`), `eth0` is the ISP network (default route), automatic mode. */
export function routerState(opts = {}) {
  return build({ router: true, via: "br-lan", networks: [BR_LAN(), ETH0()], ...split(opts, { access: "localnet", webAccess: "localnet" }) });
}

/** Router with a guest network: the saved list is `["br-lan"]`, `br-guest` and `eth0` are closed (`by: "list"`). */
export function routerGuestState(opts = {}) {
  return build({
    router: true, via: "br-lan", networks: [BR_LAN(), BR_GUEST(), ETH0()],
    ...split(opts, { access: "localnet", ifaces: ["br-lan"], webAccess: "localnet" }),
  });
}

/** Laptop: a single network `en0` (default route), the page is opened on the machine itself (`via_iface: null`), the proxy is only for it. */
export function laptopState(opts = {}) {
  return build({ router: false, via: null, networks: [EN0()], ...split(opts, { access: "localhost", webAccess: "localhost" }) });
}

/** Laptop with a VPN: `en0` and `utun3` (`kind: "tunnel"`). */
export function laptopVpnState(opts = {}) {
  return build({ router: false, via: null, networks: [EN0(), UTUN3()], ...split(opts, { access: "localhost", webAccess: "localhost" }) });
}

/** A config consistent with the snapshot: the saved `socks.access`, `socks.ifaces`, `web.access`, `web.ifaces` (`[]` — automatic). */
export function netsConfig(state) {
  const saved = (list) => (Array.isArray(list) ? [...list] : []);
  return demoConfig({
    socks: { enabled: true, port: PORT, access: state.socks.access, ifaces: saved(state.socks.ifaces) },
    web: { access: state.web.access, ifaces: saved(state.web.ifaces) },
  });
}
