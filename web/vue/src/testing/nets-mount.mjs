/* Mounting the network selection component on a fake node: the store from a snapshot, `fetch` is `createFakeNode`, so the write
 * (`PUT /api/config`), `restart_pending` and `POST /api/actions/restart` go through the real `api.ts` and `save-config.ts`. */

import { flushPromises, mount } from "@vue/test-utils";
import { vi } from "vitest";
import { createStore, STORE_KEY } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { createFakeNode } from "./fake-node.mjs";
import { netsConfig } from "./nets-states.mjs";
import { applySnapshot } from "./snapshot.mjs";

const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };

/** `opts`: `props`, `config` (otherwise `netsConfig(state)`), `configFile`. Returns `{ wrapper, store, node }`. */
export function mountWithNode(Component, state, opts = {}) {
  const node = createFakeNode({ state, config: opts.config ?? netsConfig(state) });
  vi.stubGlobal("fetch", node.fetch);
  localStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  applySnapshot(store, { state, config: node.config, configRev: node.rev, configMeta: META, configFile: opts.configFile });
  const wrapper = mount(Component, { props: opts.props, attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return { wrapper, store, node };
}

export const settled = () => flushPromises();

/** The last PUT /api/config of the node: the request body. */
export function lastPut(node) {
  return node.calls.filter((c) => c.method === "PUT" && c.path === "/api/config").at(-1)?.body;
}

/** Element text with a single space between words (the markup splits lines into nodes). */
export const flat = (wrapper) => wrapper.text().replace(/\s+/g, " ").trim();
