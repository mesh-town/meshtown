/* The whole page for tests: `AppShell` with a store, as in `main.js`, with the node's fake `fetch` and vitest timers. */

import { flushPromises, mount } from "@vue/test-utils";
import { expect, vi } from "vitest";
import { nextTick } from "vue";
import { TOKEN_KEY } from "../core/token.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { POLL_STOPPED_MS } from "../shell/conn.ts";
import { defaultShellDeps } from "../shell/shell-deps.ts";
import AppShell from "../shell/AppShell.vue";
import { adoptUrlToken } from "../shell/token-url.ts";

const STEP_MS = 500;
const READY_TRIES = 20;
const TOKEN = "test-token";
let current = null;

/** Flushes microtasks and Vue ticks. */
export async function settle() {
  await flushPromises();
  await nextTick();
  await flushPromises();
}

async function waitReady(wrapper) {
  for (let i = 0; i < READY_TRIES; i += 1) {
    if (wrapper.find('[data-ready="true"]').exists()) return;
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  }
  throw new Error("the page did not set data-ready");
}

function prepareUrl({ hash, token, urlToken }) {
  sessionStorage.clear();
  history.replaceState(null, "", "/" + (urlToken ? "?token=" + urlToken : ""));
  location.hash = hash || "";
  if (!urlToken) sessionStorage.setItem(TOKEN_KEY, token);
  adoptUrlToken(defaultShellDeps);
}

/** Mounts the page on the node `node` and waits for `data-ready` (`ready: false` — do not wait: the sign-in or first-password screen). */
export async function mountPage(node, { hash = "", token = TOKEN, urlToken = null, ready = true } = {}) {
  vi.useFakeTimers();
  vi.stubGlobal("fetch", node.fetch);
  prepareUrl({ hash, token, urlToken });
  const store = createStore();
  const wrapper = mount(AppShell, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  current = { wrapper, node };
  if (ready) await waitReady(wrapper);
  else await vi.advanceTimersByTimeAsync(0).then(settle);
  return { wrapper, store };
}

const cycles = (node) => node.count("GET", "/api/overview") + node.count("GET", "/api/state");

/** Advances timers until the number of poll cycles (`GET /api/overview`, or `GET /api/state` of a node without it) grows by `n`. */
export async function polls(node, n) {
  const want = cycles(node) + n;
  for (let spent = 0; cycles(node) < want; spent += STEP_MS) {
    if (spent > n * POLL_STOPPED_MS) throw new Error("polling of the node did not reach +" + n);
    await vi.advanceTimersByTimeAsync(STEP_MS);
  }
  await settle();
}

/** Clicks a section tab; happy-dom does not follow a `#` link itself, so the hash and `hashchange` are delivered explicitly. */
export async function openTab(wrapper, section) {
  const tab = wrapper.get("#tab-" + section);
  await tab.trigger("click");
  if (location.hash !== "#" + section) location.hash = section;
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  await vi.advanceTimersByTimeAsync(0);
  await settle();
}

/** Unmounts and checks that the node received no unknown requests. */
export function unmountPage() {
  const { wrapper, node } = current;
  current = null;
  wrapper.unmount();
  vi.useRealTimers();
  location.hash = "";
  expect(node.unexpected).toEqual([]);
}
