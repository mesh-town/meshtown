/* A port whose local socket is still being taken (`state: "binding"`) shows a grey "opening…" badge instead of the
 * green "open" one, keeps "Close" and has no "Change number"; the next snapshot turns it green or shows the failure. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const BUSY = "port 5353 is busy — choose another number";
const BIND_ERROR = "127.0.0.1:5353 cannot be bound: Address in use (os error 98)";

const KEY = { tcp: "ab/53", udp: "ab/53/udp" };
const entry = (proto) => ({ node: "ab", port: 53, ...(proto === "udp" ? { proto } : {}), local: 5353 });
const opened = (over = {}) => ({ local: 5353, state: "listening", error: null, last_error: null, ...over });
const netRow = (proto, over = {}) => ({ node: "ab", node_name: "finch", port: 53, proto, name: "", published: true, online: true, opened: null, web_url: null, ...over });
const snapshot = (store, row) => applySnapshot(store, { state: baseState({ ports: { published: [], network: [row] } }) });

let mounted;
const mountNet = (row, proto) => (mounted = mountSection(PortsSection, {
  props: { slot: "main" },
  state: baseState({ ports: { published: [], network: [row] } }),
  config: baseConfig({ ports: { published: [], opened: row.opened ? [entry(proto)] : [] } }),
}));
const rowOf = (wrapper, proto) => wrapper.get('[data-port-row="' + KEY[proto] + '"]');
const badges = (wrapper, proto) => rowOf(wrapper, proto).findAll(".badge").map((b) => b.text());
const isGreen = (wrapper, proto) => rowOf(wrapper, proto).findAll(".badge-ok").some((b) => b.text() === "open");

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

for (const proto of ["udp", "tcp"]) {
  describe(proto + " row in the binding state", () => {
    it('binding shows a grey "opening…" badge', () => {
      const { wrapper } = mountNet(netRow(proto, { opened: opened({ state: "binding" }) }), proto);
      const row = rowOf(wrapper, proto);
      expect(badges(wrapper, proto)).toContain("opening…");
      expect(badges(wrapper, proto)).not.toContain("open");
      expect(row.findAll(".badge-ok").some((b) => b.text() === "opening…")).toBe(false);
      expect(isGreen(wrapper, proto)).toBe(false);
      expect(row.find('[data-port-close="' + KEY[proto] + '"]').exists()).toBe(true);
      expect(row.find('[data-port-change="' + KEY[proto] + '"]').exists()).toBe(false);
    });

    it("binding then listening turns green", async () => {
      const { wrapper, store } = mountNet(netRow(proto, { opened: opened({ state: "binding" }) }), proto);
      expect(isGreen(wrapper, proto)).toBe(false);
      snapshot(store, netRow(proto, { opened: opened() }));
      await flushPromises();
      expect(isGreen(wrapper, proto)).toBe(true);
      expect(badges(wrapper, proto)).not.toContain("opening…");
      expect(rowOf(wrapper, proto).find('[data-port-change="' + KEY[proto] + '"]').exists()).toBe(true);
    });

    it("binding then bind_failed shows the failure", async () => {
      const { wrapper, store } = mountNet(netRow(proto, { opened: opened({ state: "binding" }) }), proto);
      snapshot(store, netRow(proto, { opened: opened({ state: "bind_failed", error: BIND_ERROR }) }));
      await flushPromises();
      const text = rowOf(wrapper, proto).text();
      expect(text).toContain(BUSY);
      expect(text).toContain("Couldn't open: " + BIND_ERROR);
      expect(badges(wrapper, proto)).not.toContain("opening…");
      expect(rowOf(wrapper, proto).find('[data-port-open="' + KEY[proto] + '"]').exists()).toBe(true);
    });
  });
}
