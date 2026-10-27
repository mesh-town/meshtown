/* Helpers for the setup wizard component tests: mounting `SetupWizard` on a node snapshot with the preconditions
 * written as options (network, app, proxy, setup mark, remembered scope, user agent), reading the screen and clicking through it. */

import { flushPromises } from "@vue/test-utils";
import { vi } from "vitest";
import SetupWizard from "../home/SetupWizard.vue";
import { baseConfig, baseState, mountSection } from "./section.mjs";

export const MARK_KEY = "meshtown.homeSetup";
export const SCOPE_KEY = "meshtown.proxyScope";
export const PAC_URL = "http://127.0.0.1:1080/proxy.pac";

export const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
export const WINDOWS_EDGE = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
export const LINUX_FIREFOX = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
export const ANDROID_CHROME = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
export const IPHONE_SAFARI = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
export const APP_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android";

const START_ERROR = "port 1091 for the browser proxy is in use";

/** `localStorage` that reads but throws on every write; `data` is what a test seeds before mounting. */
export function brokenStorage() {
  const data = new Map();
  const fail = () => { throw new Error("localStorage is unavailable"); };
  return { data, getItem: (k) => (data.has(k) ? data.get(k) : null), setItem: fail, removeItem: fail, clear: fail };
}

function snapshot({ hasNet, proxyOn, startError, label, over, state }) {
  if (state) return state;
  const socks = proxyOn ? { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: PAC_URL } } : {};
  const network = hasNet ? (label ? { network: { id_hex: "net-aaaa", label } } : {}) : { network: null, peers: [] };
  const node = { ...baseState().node, ...(startError ? { start_error: START_ERROR } : {}), ...(over.node ?? {}) };
  return baseState({ ...socks, ...network, ...over, node });
}

/**
 * Mounts the wizard. `hasNet`/`proxyOn` build the snapshot (`over` — extra top-level fields, `state` — a whole snapshot instead);
 * `inApp` sets the app user agent; `mark` and `scope` are written to the page storage first; `storage` (a `brokenStorage()`) replaces it.
 * → `{ wrapper, store, storage }`; pair every call with `unmountWizard`.
 */
export function mountWizard({ hasNet = true, inApp = false, proxyOn = true, mark = null, scope = null, ua, startError = false, label, over = {}, state, config, storage } = {}) {
  localStorage.clear();
  const seed = { [MARK_KEY]: mark, [SCOPE_KEY]: scope };
  const target = storage ?? localStorage;
  for (const [key, value] of Object.entries(seed)) if (value) (storage ? storage.data.set(key, value) : localStorage.setItem(key, value));
  if (storage) vi.stubGlobal("localStorage", storage);
  Object.defineProperty(navigator, "userAgent", { value: ua ?? (inApp ? APP_UA : MAC_CHROME), configurable: true });
  const mounted = mountSection(SetupWizard, { state: snapshot({ hasNet, proxyOn, startError, label, over, state }), config: config ?? baseConfig() });
  return { ...mounted, storage: target };
}

export function unmountWizard(mounted) {
  mounted?.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
  Reflect.deleteProperty(navigator, "clipboard");
}

/** The id of the screen the wizard shows now (`data-step` of `#wizard-screen`). */
export const step = (wrapper) => wrapper.get("#wizard-screen").attributes("data-step");

/** The text of the step indicator, e.g. "Step 2 of 6". */
export const progress = (wrapper) => wrapper.get("#wizard-progress").text();

/** Texts of the buttons of a tile group (`#scope-seg`, `#os-seg`, `#browser-seg`). */
export const tiles = (wrapper, group) => wrapper.findAll("#" + group + " button").map((b) => b.text());

/** Names of the pressed tiles of a group: `["chrome"]` for a pressed `#browser-chrome` in `browser-seg`. */
export const pressedTiles = (wrapper, group) => {
  const prefix = group.replace("-seg", "-");
  return wrapper.findAll("#" + group + " button").filter((b) => b.attributes("aria-pressed") === "true").map((b) => b.attributes("id").slice(prefix.length));
};

/** Clicks an element and lets the wizard settle (the step change, the focus move). */
export async function click(wrapper, selector) {
  await wrapper.get(selector).trigger("click");
  await flushPromises();
}

/**
 * Walks forward to the guide screen: on the os and browser screens the tile of `os`/`browser` is clicked when given,
 * everywhere else Next is pressed (so the scope must be remembered or the walk fails on a missing Next).
 */
export async function toGuide(wrapper, { os, browser } = {}) {
  for (let i = 0; i < 6 && step(wrapper) !== "guide"; i += 1) {
    const here = step(wrapper);
    const tile = here === "os" ? os : here === "browser" ? browser : undefined;
    await click(wrapper, tile ? "#" + here + "-" + tile : "#wizard-next");
  }
}

/** Element text without the hover tips of the terms, whitespace collapsed. */
export function plain(el) {
  const copy = el.element.cloneNode(true);
  copy.querySelectorAll('[role="tooltip"]').forEach((tip) => tip.remove());
  return copy.textContent.replace(/\s+/g, " ").trim();
}
