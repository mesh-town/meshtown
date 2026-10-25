/* The network name for a human per the naming table: rules in order, the first matching one wins. */

import {
  NETS_IFACE_GUEST_TITLE, NETS_IFACE_HOME_HINT, NETS_IFACE_HOME_TITLE, NETS_IFACE_PROVIDER_HINT, NETS_IFACE_PROVIDER_TITLE,
  NETS_IFACE_VM_TITLE, NETS_IFACE_VPN_TITLE, netsIfaceOtherTitleText,
} from "./nets-texts.ts";
import type { NetEntry } from "./nets-types.ts";

interface IfaceTitle {
  title: string;
  hint: string;
  code: string;
}

const PROVIDER_HINT = NETS_IFACE_PROVIDER_HINT;
const HOME_HINT = NETS_IFACE_HOME_HINT;
const VM_TITLE = NETS_IFACE_VM_TITLE;
const TUNNEL_NAME = /^(utun|tun|wg|tailscale)/;
const VIRTUAL_NAME = /^(docker|veth|vmnet|vboxnet|bridge)/;

type Rule = (e: NetEntry, router: boolean, all: NetEntry[]) => { title: string; hint?: string } | null;

const isHomeName = (e: NetEntry, all: NetEntry[]): boolean =>
  /^(br-lan|lan)$/.test(e.name) || all.filter((x) => !x.default_route).length === 1;

const isTunnel = (e: NetEntry): boolean => e.kind === "tunnel" || TUNNEL_NAME.test(e.name);
const isVirtual = (e: NetEntry, router: boolean): boolean =>
  e.kind === "virtual" || VIRTUAL_NAME.test(e.name) || (!router && /^br-/.test(e.name));

const RULES: Rule[] = [
  (e, r) => (r && e.default_route ? { title: NETS_IFACE_PROVIDER_TITLE, hint: PROVIDER_HINT } : null),
  (e, r, all) => (r && isHomeName(e, all) ? { title: NETS_IFACE_HOME_TITLE, hint: HOME_HINT } : null),
  (e) => (/guest/i.test(e.name) ? { title: NETS_IFACE_GUEST_TITLE } : null),
  (e) => (isTunnel(e) ? { title: NETS_IFACE_VPN_TITLE } : null),
  (e, r) => (isVirtual(e, r) ? { title: VM_TITLE } : null),
  (e, r) => (!r && e.default_route ? { title: NETS_IFACE_HOME_TITLE } : null),
  (e) => ({ title: netsIfaceOtherTitleText(e.name) }),
];

function codeOf(e: NetEntry): string {
  const addr = e.addrs[0];
  return addr ? e.name + " · " + addr : e.name;
}

export function ifaceTitle(entry: NetEntry, router: boolean, all: NetEntry[]): IfaceTitle {
  const hit = RULES.map((rule) => rule(entry, router, all)).find((x) => x !== null);
  return { title: hit?.title ?? "", hint: hit?.hint ?? "", code: codeOf(entry) };
}
