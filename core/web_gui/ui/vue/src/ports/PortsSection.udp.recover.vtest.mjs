/* Every failure shown on a shared port row (`last_error` of a TCP or UDP row, `bind_failed` of either) turns the row's
 * indicator from green to an error text, and the next snapshot without the failure clears both. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const TEXT = {
  unsupported: "The member runs an old version: UDP is not supported",
  offline: "The member is offline",
  not_opened: "The member has not shared this port with you",
  no_service: "Nothing is listening on the member's port",
  timed_out: "The member did not answer in time",
  failed: "Couldn't reach the member's port",
};
const BUSY = "port 5353 is busy — choose another number";
const BIND_ERROR = "127.0.0.1:5353 cannot be bound: Address in use (os error 98)";

const KEY = { tcp: "ab/53", udp: "ab/53/udp" };
const entry = (proto) => ({ node: "ab", port: 53, ...(proto === "udp" ? { proto } : {}), local: 5353 });
const opened = (over = {}) => ({ local: 5353, state: "listening", error: null, last_error: null, ...over });
const netRow = (proto, over = {}) => ({ node: "ab", node_name: "finch", port: 53, proto, name: "", published: true, online: true, opened: null, web_url: null, ...over });
const snapshot = (store, row) => applySnapshot(store, { state: baseState({ ports: { published: [], network: [row] } }) });

let mounted;
const mountNet = (row, proto) => (mounted = mountSection(PortsSection, {
  state: baseState({ ports: { published: [], network: [row] } }),
  config: baseConfig({ ports: { published: [], opened: row.opened ? [entry(proto)] : [] } }),
}));
const rowOf = (wrapper, proto) => wrapper.get('[data-port-row="' + KEY[proto] + '"]');
const lastErrorOf = (wrapper, proto) => wrapper.find('[data-port-last-error="' + KEY[proto] + '"]');
const isGreen = (wrapper, proto) => rowOf(wrapper, proto).findAll(".badge-ok").some((b) => b.text() === "open");

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

describe("a failure on a shared port row and its recovery", () => {
  const cases = [];
  for (const proto of ["tcp", "udp"]) {
    for (const [code, text] of Object.entries(TEXT)) {
      cases.push([proto, code, proto === "tcp" && code === "unsupported" ? TEXT.failed : text]);
    }
  }

  for (const [proto, code, text] of cases) {
    it(proto + " last_error " + code + ": no green indicator, the text shows, a later success clears it", async () => {
      const { wrapper, store } = mountNet(netRow(proto, { opened: opened({ last_error: code }) }), proto);
      expect(lastErrorOf(wrapper, proto).text()).toBe(text);
      expect(isGreen(wrapper, proto)).toBe(false);
      snapshot(store, netRow(proto, { opened: opened() }));
      await flushPromises();
      expect(lastErrorOf(wrapper, proto).exists()).toBe(false);
      expect(isGreen(wrapper, proto)).toBe(true);
    });
  }

  for (const proto of ["tcp", "udp"]) {
    it(proto + " bind_failed: no green indicator and both messages show, a later listening state clears them", async () => {
      const failed = opened({ state: "bind_failed", error: BIND_ERROR });
      const { wrapper, store } = mountNet(netRow(proto, { opened: failed }), proto);
      const text = rowOf(wrapper, proto).text();
      expect(text).toContain(BUSY);
      expect(text).toContain("Couldn't open: " + BIND_ERROR);
      expect(isGreen(wrapper, proto)).toBe(false);
      snapshot(store, netRow(proto, { opened: opened() }));
      await flushPromises();
      expect(rowOf(wrapper, proto).text()).not.toContain(BUSY);
      expect(rowOf(wrapper, proto).text()).not.toContain("Couldn't open");
      expect(isGreen(wrapper, proto)).toBe(true);
    });
  }

  for (const proto of ["tcp", "udp"]) {
    it(proto + ": open, fail, recover, close leaves a plain row with no error text", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper, store } = mountNet(netRow(proto), proto);
      await wrapper.get('[data-port-open="' + KEY[proto] + '"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls.at(-1)[2].config.ports.opened).toEqual([{ ...entry(proto), local: 2053 }]);
      snapshot(store, netRow(proto, { opened: opened({ last_error: "offline" }) }));
      await flushPromises();
      expect(lastErrorOf(wrapper, proto).text()).toBe(TEXT.offline);
      expect(isGreen(wrapper, proto)).toBe(false);
      snapshot(store, netRow(proto, { opened: opened() }));
      await flushPromises();
      expect(lastErrorOf(wrapper, proto).exists()).toBe(false);
      expect(isGreen(wrapper, proto)).toBe(true);
      await wrapper.get('[data-port-close="' + KEY[proto] + '"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls.at(-1)[2].config.ports.opened).toEqual([]);
      snapshot(store, netRow(proto));
      await flushPromises();
      expect(lastErrorOf(wrapper, proto).exists()).toBe(false);
      expect(isGreen(wrapper, proto)).toBe(false);
    });
  }
});
