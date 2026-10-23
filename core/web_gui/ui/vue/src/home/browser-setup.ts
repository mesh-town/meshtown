/* The manual block "Connect the browser to Mesh Town": the browser and system by `userAgent`, numbered steps for
 * them, the proxy address and port from `state.socks.listen` (a port of `07b-browser-setup.js`). Pure
 * functions. */

import type { NodeState } from "../core/api-types.ts";
import { PAC_INTRO, PHONE_SOON } from "../common/pac-texts.ts";
import { lanView, routerPacUrl, type LanView } from "../lan/nets-view.ts";
import { PAC_INTRO_ROUTER } from "../lan/nets-texts.ts";
import {
  HOME_BROWSER_WARN, HOME_FF_DNS, HOME_FF_MANUAL, HOME_FF_OPEN, HOME_FF_SOCKS5, HOME_LINUX_MANUAL, HOME_LINUX_OPEN, HOME_MAC_DETAILS,
  HOME_MAC_OK, HOME_MAC_OPEN, HOME_MAC_SOCKS, HOME_SAFARI_ONLY_MAC, HOME_STEP_HOST, HOME_STEP_PORT, HOME_STEP_SOCKS_HOST, HOME_WIN_FILL, HOME_WIN_MANUAL,
  HOME_WIN_OPEN, HOME_WIN_SAVE,
} from "./home-flow-texts.ts";
import { pacBlock, type PacBlock } from "./pac-setup.ts";

export type BrowserId = "chrome" | "edge" | "safari" | "firefox";
export type OsId = "windows" | "mac" | "linux" | "phone";
export type StepValue = "host" | "port" | "lan" | "pac";
/** The page is open inside the Android app: its WebView appends `MeshTownApp/` to the user agent. */
export const isAppUserAgent = (userAgent: string): boolean => userAgent.includes("MeshTownApp/");
export const inAppPage = (): boolean => typeof navigator !== "undefined" && isAppUserAgent(navigator.userAgent || "");

export interface BrowserStep {
  text: string;
  copyKind?: "host" | "port";
}
export interface HostPort {
  host: string;
  port: string;
}
/** `lan` — addresses for the network's other devices (`192.168.8.1:1080` and the interface name); no key while the proxy is closed to the network. */
export interface LanAddr {
  addr: string;
  iface: string;
}
export type ProxyView = { on: false } | ({ on: true; lan?: LanAddr[]; pacUrl: string } & HostPort);
export type ProxyState = { on: false } | ({
  on: true; lan?: LanAddr[]; pacUrl: string; intro: string; lanView: LanView | null; browser: BrowserId; steps: BrowserStep[]; warn: string; os: OsId; pac: PacBlock;
} & HostPort);

export const BROWSER_IDS: BrowserId[] = ["chrome", "edge", "safari", "firefox"];

const BROWSER_WARN = HOME_BROWSER_WARN;
const SAFARI_ONLY_MAC = HOME_SAFARI_ONLY_MAC;

export function detectBrowser(ua: string): BrowserId {
  if (/Edg(e|A|iOS)?\//.test(ua)) return "edge";
  if (/Firefox\/|FxiOS\//.test(ua)) return "firefox";
  if (/Chrome\/|CriOS\//.test(ua)) return "chrome";
  return /Safari\//.test(ua) ? "safari" : "firefox";
}

export function detectOs(ua: string): OsId {
  if (/iPhone|iPad|iPod|Android/.test(ua)) return "phone";
  if (/Windows/.test(ua)) return "windows";
  return /Macintosh|Mac OS X/.test(ua) ? "mac" : "linux";
}

const HOST_PORT: BrowserStep[] = [{ text: HOME_STEP_HOST, copyKind: "host" }, { text: HOME_STEP_PORT, copyKind: "port" }];

function firefoxSteps(): BrowserStep[] {
  return [
    { text: HOME_FF_OPEN },
    { text: HOME_FF_MANUAL },
    { text: HOME_STEP_SOCKS_HOST, copyKind: "host" }, { text: HOME_STEP_PORT, copyKind: "port" },
    { text: HOME_FF_SOCKS5 },
    { text: HOME_FF_DNS },
  ];
}

function macSteps(): BrowserStep[] {
  return [
    { text: HOME_MAC_OPEN },
    { text: HOME_MAC_DETAILS },
    { text: HOME_MAC_SOCKS },
    ...HOST_PORT,
    { text: HOME_MAC_OK },
  ];
}

function windowsSteps(): BrowserStep[] {
  return [
    { text: HOME_WIN_OPEN },
    { text: HOME_WIN_MANUAL },
    { text: HOME_WIN_FILL },
    ...HOST_PORT,
    { text: HOME_WIN_SAVE },
  ];
}

function linuxSteps(): BrowserStep[] {
  return [
    { text: HOME_LINUX_OPEN },
    { text: HOME_LINUX_MANUAL },
    ...HOST_PORT,
  ];
}

const OS_STEPS = new Map<OsId, () => BrowserStep[]>([["mac", macSteps], ["windows", windowsSteps]]);

export function browserSteps(browser: BrowserId, os: OsId): BrowserStep[] {
  if (os === "phone") return [{ text: PHONE_SOON }];
  if (browser === "firefox") return firefoxSteps();
  if (browser === "safari" && os !== "mac") return [{ text: SAFARI_ONLY_MAC }];
  return (OS_STEPS.get(os) || linuxSteps)();
}

// The warning — only where the steps change a system setting: not for Firefox and not for
// Safari outside macOS.
export function browserWarnText(browser: BrowserId, os: OsId): string {
  if (browser === "firefox" || os === "phone") return "";
  return browser === "safari" && os !== "mac" ? "" : BROWSER_WARN;
}

// By the last ":"; IPv6 comes in brackets (`[::1]:1080`) — we strip the brackets.
export function splitHostPort(listen: string): HostPort {
  const idx = listen.lastIndexOf(":");
  if (idx === -1) return { host: listen, port: "" };
  const host = listen.slice(0, idx);
  const bracketed = host.startsWith("[") && host.endsWith("]");
  return { host: bracketed ? host.slice(1, -1) : host, port: listen.slice(idx + 1) };
}

// `{ on: false }` — the proxy is off or there is no address; otherwise the address and port for the steps.
export function proxyView(state: Partial<NodeState> | null): ProxyView {
  const socks = state && state.socks;
  if (!socks || !socks.enabled || !socks.listen) return { on: false };
  const routerPac = routerPacUrl(state);
  const pacUrl = routerPac ?? (socks.pac_url || "http://" + socks.listen + "/proxy.pac");
  return { on: true, ...splitHostPort(routerPac ? new URL(routerPac).host : socks.listen), pacUrl, ...(socks.lan && socks.lan.length ? { lan: socks.lan } : {}) };
}

function proxyDetails(base: { on: true; pacUrl: string } & HostPort, state: Partial<NodeState>, choice: BrowserId | null, ua: string, osChoice: OsId | null): ProxyState {
  const browser = choice || detectBrowser(ua);
  const os = osChoice || detectOs(ua);
  return { ...base, intro: routerPacUrl(state) ? PAC_INTRO_ROUTER : PAC_INTRO, lanView: lanView(state), browser, steps: browserSteps(browser, os), warn: browserWarnText(browser, os), os, pac: pacBlock(os) };
}

// The proxy view for the card: without an enabled proxy — as is, otherwise with the browser (chosen or by `ua`) and steps.
export function proxyState(state: Partial<NodeState> | null, choice: BrowserId | null, ua: string, osChoice: OsId | null = null): ProxyState {
  const base = proxyView(state || {});
  return base.on ? proxyDetails(base, state || {}, choice, ua, osChoice) : base;
}
