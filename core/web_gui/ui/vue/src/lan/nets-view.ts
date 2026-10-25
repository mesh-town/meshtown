/* Pure functions of the network selection over a node snapshot (`NetsInput`) and the saved config (`SavedNets`). */

import { ifaceTitle } from "./iface-title.ts";
import { LAN_HINT_MULTI, NETS_IFACE_HOME_TITLE, NETS_LEGACY_LAN_HINT, NETS_PROVIDER_CLOSED_AUTO, netsClosedForText, netsSameAsProxyText } from "./nets-texts.ts";
import type { NetEntry, NetsInput, SavedNets } from "./nets-types.ts";

export type { NetsInput, SavedNets } from "./nets-types.ts";

export const AUTO_PAYLOAD = { "socks.ifaces": [] as string[], "web.ifaces": [] as string[] };
const HOME_TITLE = NETS_IFACE_HOME_TITLE;
const LEGACY_LAN_HINT = NETS_LEGACY_LAN_HINT;

interface LanItem {
  title: string;
  addr: string;
}
export interface LanView {
  kind: "legacy" | "home" | "single" | "multi";
  items: LanItem[];
  hint: string;
}

export const isRouter = (state: NetsInput): boolean => state.node?.router === true;
export const networksOf = (state: NetsInput): NetEntry[] => state.node?.networks ?? [];
export const viaIface = (state: NetsInput): string | null => state.node?.via_iface ?? null;

const titleOf = (state: NetsInput, entry: NetEntry): string => ifaceTitle(entry, isRouter(state), networksOf(state)).title;

/** Whether a network is open by the saved list; list not set — automatic (on a router the ISP network is closed). */
export function openBySaved(entry: NetEntry, saved: string[] | null | undefined, router: boolean): boolean {
  if (saved && saved.length > 0) return saved.includes("*") || saved.includes(entry.name);
  return !(router && entry.default_route);
}

export function draftNames(state: NetsInput, saved: string[] | null | undefined): string[] {
  const router = isRouter(state);
  return networksOf(state).filter((n) => openBySaved(n, saved, router)).map((n) => n.name);
}

export function isDirty(draft: string[], state: NetsInput, saved: string[] | null | undefined): boolean {
  const base = draftNames(state, saved);
  return draft.length !== base.length || draft.some((name) => !base.includes(name));
}

/** Where the networks block stands: in the main settings, in the "Page" section or nowhere (fewer than two networks). */
export function blockWhere(config: SavedNets, state: NetsInput): "main" | "page" | null {
  if (networksOf(state).length < 2) return null;
  if (config.socks?.access === "localnet") return "main";
  return config.web?.access === "localnet" ? "page" : null;
}

export function sameNetsLine(config: SavedNets, state: NetsInput): string | null {
  if (blockWhere(config, state) !== "main" || config.web?.access !== "localnet") return null;
  const router = isRouter(state);
  const open = networksOf(state).filter((n) => openBySaved(n, config.socks?.ifaces, router));
  return netsSameAsProxyText(open.map((n) => titleOf(state, n)));
}

export function summaryView(state: NetsInput, saved: string[] | null | undefined): { open: string[]; tail: string } {
  const router = isRouter(state);
  const all = networksOf(state);
  const open = all.filter((n) => openBySaved(n, saved, router)).map((n) => titleOf(state, n));
  const closed = all.filter((n) => !openBySaved(n, saved, router)).map((n) => titleOf(state, n));
  return { open, tail: tailOf(saved, closed) };
}

function tailOf(saved: string[] | null | undefined, closed: string[]): string {
  if (closed.length === 0) return "";
  return saved && saved.length > 0 ? netsClosedForText(closed) : NETS_PROVIDER_CLOSED_AUTO;
}

function lanItems(state: NetsInput): LanItem[] {
  const all = networksOf(state);
  return (state.socks?.lan ?? []).map((l) => {
    const entry = all.find((n) => n.name === l.iface);
    return { title: entry ? titleOf(state, entry) : l.iface, addr: l.addr };
  });
}

const LOOPBACK_PAC = /^https?:\/\/(127\.0\.0\.1|localhost|\[::1\])(:\d+)?(\/|$)/;

/** The auto-configuration address on a router: `socks.pac_url` if the node sent the address of the request's network (not loopback), else
 * the "Home network" from `socks.lan`, else the first open one; not a router or no address — `null`. */
export function routerPacUrl(state: NetsInput): string | null {
  const own = state.socks?.pac_url;
  if (!isRouter(state)) return null;
  if (own && !LOOPBACK_PAC.test(own)) return own;
  const items = lanItems(state);
  if (items.length === 0) return null;
  const pick = items.find((i) => i.title === HOME_TITLE) ?? items[0];
  return "http://" + pick.addr + "/proxy.pac";
}

/** The address line for other devices; `null` — the proxy is closed from the network. */
export function lanView(state: NetsInput): LanView | null {
  const items = lanItems(state);
  if (state.socks?.access !== "localnet" || items.length === 0) return null;
  if (!state.node?.networks) return { kind: "legacy", items, hint: items.length > 1 ? LEGACY_LAN_HINT : "" };
  if (items.length > 1) return { kind: "multi", items, hint: LAN_HINT_MULTI };
  const home = isRouter(state) && items[0].title === HOME_TITLE;
  return { kind: home ? "home" : "single", items, hint: "" };
}

export function savePayload(names: string[]): { "socks.ifaces": string[]; "web.ifaces": string[] } {
  return { "socks.ifaces": [...names], "web.ifaces": [...names] };
}
