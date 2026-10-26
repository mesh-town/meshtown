/* "Ports shared with you" for the two protocols: TCP 53 and UDP 53 of one member are two rows (`ab/53`, `ab/53/udp`), the UDP
 * row has a badge and no browser address, Open and Close write the entry of their own protocol, and the member's last
 * error is shown under the row. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const TCP_KEY = "ab/53";
const UDP_KEY = "ab/53/udp";
const TCP_OPENED = { node: "ab", port: 53, local: 2053 };
const UDP_OPENED = { node: "ab", port: 53, proto: "udp", local: 5353 };
const listening = (local, last_error = null) => ({ local, state: "listening", error: null, last_error });

const tcpRow = (over = {}) => ({ node: "ab", node_name: "finch", port: 53, proto: "tcp", name: "", published: true, online: true, opened: null, web_url: null, ...over });
const udpRow = (over = {}) => ({ ...tcpRow(), proto: "udp", ...over });

let mounted;
const mountNet = (network, opened = []) => (mounted = mountSection(PortsSection, {
  state: baseState({ ports: { published: [], network } }),
  config: baseConfig({ ports: { published: [], opened } }),
}));
const lastOpened = () => api.mock.calls.at(-1)[2].config.ports.opened;
const okApi = () => api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
const rowOf = (wrapper, key) => wrapper.get('[data-port-row="' + key + '"]');
const lastErrorOf = (wrapper, key) => wrapper.find('[data-port-last-error="' + key + '"]');

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

describe("member ports with the same number in TCP and UDP", () => {
  describe("the rows", () => {
    it("are two, addressed ab/53 and ab/53/udp", () => {
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      expect(wrapper.findAll("#ports-net-tbody tr")).toHaveLength(2);
      expect(wrapper.find('[data-port-row="' + TCP_KEY + '"]').exists()).toBe(true);
      expect(wrapper.find('[data-port-row="' + UDP_KEY + '"]').exists()).toBe(true);
    });

    it("each has its own number field, Open button and default local number", () => {
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      for (const key of [TCP_KEY, UDP_KEY]) {
        expect(wrapper.get('[data-port-local="' + key + '"]').element.value, key).toBe("2053");
        expect(wrapper.find('[data-port-open="' + key + '"]').exists(), key).toBe(true);
      }
    });

    it("the UDP badge is only on the UDP row", () => {
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      expect(rowOf(wrapper, TCP_KEY).find("[data-port-proto]").exists()).toBe(false);
      const badge = rowOf(wrapper, UDP_KEY).get('[data-port-proto="udp"]');
      expect(badge.text()).toBe("UDP");
      expect(badge.classes()).toEqual(expect.arrayContaining(["badge", "badge-grey"]));
    });

    it("the TCP port cell is the bare number", () => {
      const { wrapper } = mountNet([tcpRow({ port: 8080 })]);
      expect(rowOf(wrapper, "ab/8080").findAll("td")[1].text()).toBe("8080");
    });

    it("the UDP port cell is the number and the badge", () => {
      const { wrapper } = mountNet([udpRow({ port: 8080 })]);
      expect(rowOf(wrapper, "ab/8080/udp").findAll("td")[1].text()).toMatch(/^8080\s*UDP$/);
    });

    it("an entry without proto is a TCP row", () => {
      const { proto: _p, ...legacy } = tcpRow();
      const { wrapper } = mountNet([legacy]);
      expect(wrapper.find('[data-port-row="' + TCP_KEY + '"]').exists()).toBe(true);
      expect(wrapper.find("[data-port-proto]").exists()).toBe(false);
    });
  });

  describe("the browser column", () => {
    it("a TCP row on a browser-blocked port says \"not for the browser\"", () => {
      const { wrapper } = mountNet([tcpRow()]);
      expect(rowOf(wrapper, TCP_KEY).findAll("td")[2].text()).toBe("not for the browser");
    });

    it("a UDP row on the same port says nothing", () => {
      const { wrapper } = mountNet([udpRow()]);
      const cell = rowOf(wrapper, UDP_KEY).findAll("td")[2];
      expect(cell.text()).toBe("");
      expect(cell.text()).not.toContain("not for the browser");
    });

    it("a TCP row with a web address has a link and a Copy button", () => {
      const { wrapper } = mountNet([tcpRow({ port: 5000, web_url: "http://finch.meshtown:5000" })]);
      const cell = rowOf(wrapper, "ab/5000").findAll("td")[2];
      expect(cell.get("a").attributes("href")).toBe("http://finch.meshtown:5000");
    });

    it("a UDP row has no link and no Copy button even if a web address is given", () => {
      const { wrapper } = mountNet([udpRow({ port: 5000, web_url: "http://finch.meshtown:5000" })]);
      const cell = rowOf(wrapper, "ab/5000/udp").findAll("td")[2];
      expect(cell.find("a").exists()).toBe(false);
      expect(cell.find("button").exists()).toBe(false);
      expect(cell.text()).toBe("");
    });
  });

  describe("Open", () => {
    it("on the UDP row writes an entry with proto: udp and the typed number", async () => {
      okApi();
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      await wrapper.get('[data-port-local="' + UDP_KEY + '"]').setValue("5353");
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls.at(-1).slice(0, 2)).toEqual(["PUT", "/api/config"]);
      expect(lastOpened()).toEqual([UDP_OPENED]);
    });

    it("on the UDP row without a typed number writes the port plus 2000", async () => {
      okApi();
      const { wrapper } = mountNet([udpRow()]);
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([{ node: "ab", port: 53, proto: "udp", local: 2053 }]);
    });

    it("on the TCP row writes an entry without a proto field", async () => {
      okApi();
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      await wrapper.get('[data-port-open="' + TCP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([TCP_OPENED]);
      expect("proto" in lastOpened()[0]).toBe(false);
    });

    it("on the UDP row next to an opened TCP row keeps the TCP entry", async () => {
      okApi();
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053) }), udpRow()], [TCP_OPENED]);
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([TCP_OPENED, { ...UDP_OPENED, local: 2053 }]);
    });
  });

  describe("Close", () => {
    const bothOpened = () => mountNet([tcpRow({ opened: listening(2053) }), udpRow({ opened: listening(5353) })], [TCP_OPENED, UDP_OPENED]);

    it("on the UDP row removes only the UDP entry", async () => {
      okApi();
      const { wrapper } = bothOpened();
      await wrapper.get('[data-port-close="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([TCP_OPENED]);
    });

    it("on the TCP row removes only the TCP entry", async () => {
      okApi();
      const { wrapper } = bothOpened();
      await wrapper.get('[data-port-close="' + TCP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([UDP_OPENED]);
    });

    it("each opened row shows its own local address", () => {
      const { wrapper } = bothOpened();
      expect(rowOf(wrapper, TCP_KEY).text()).toContain("127.0.0.1:2053");
      expect(rowOf(wrapper, UDP_KEY).text()).toContain("127.0.0.1:5353");
    });
  });

  describe("a refused Open", () => {
    const INVALID = new ApiError(422, "invalid", "invalid", { fields: [{ path: "ports.opened", message: "Local port 5353 is already in use" }] });

    it("shows the refusal in the UDP row's error line only", async () => {
      api.mockRejectedValue(INVALID);
      const { wrapper } = mountNet([tcpRow(), udpRow()]);
      await wrapper.get('[data-port-local="' + UDP_KEY + '"]').setValue("5353");
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(wrapper.get('[data-port-error="' + UDP_KEY + '"]').text()).toBe("Local port 5353 is already in use");
      expect(wrapper.find('[data-port-error="' + TCP_KEY + '"]').exists()).toBe(false);
    });

    it("the Open button stays so that the user can try again", async () => {
      api.mockRejectedValue(INVALID);
      const { wrapper } = mountNet([udpRow()]);
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(wrapper.get('[data-port-open="' + UDP_KEY + '"]').attributes("disabled")).toBeUndefined();
    });

    it("the refusal disappears after a successful retry", async () => {
      api.mockRejectedValueOnce(INVALID);
      okApi();
      const { wrapper } = mountNet([udpRow()]);
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(wrapper.find('[data-port-error="' + UDP_KEY + '"]').exists()).toBe(true);
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(wrapper.find('[data-port-error="' + UDP_KEY + '"]').exists()).toBe(false);
    });
  });

  describe("bind_failed on a UDP row", () => {
    const failed = { local: 2053, state: "bind_failed", error: "127.0.0.1:2053 cannot be bound: Address in use (os error 98)" };

    it("shows both messages: the busy number and Couldn't open", () => {
      const { wrapper } = mountNet([udpRow({ opened: failed })], [{ ...UDP_OPENED, local: 2053 }]);
      const text = rowOf(wrapper, UDP_KEY).text();
      expect(text).toContain("port 2053 is busy — choose another number");
      expect(text).toContain("Couldn't open: 127.0.0.1:2053 cannot be bound: Address in use (os error 98)");
      expect(text).not.toContain("127.0.0.1:2053 open");
    });

    it("keeps Open and Close in the row", () => {
      const { wrapper } = mountNet([udpRow({ opened: failed })], [{ ...UDP_OPENED, local: 2053 }]);
      expect(wrapper.find('[data-port-open="' + UDP_KEY + '"]').exists()).toBe(true);
      expect(wrapper.find('[data-port-close="' + UDP_KEY + '"]').exists()).toBe(true);
    });

    it("Open on another number rewrites only the UDP entry", async () => {
      okApi();
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053) }), udpRow({ opened: failed })], [TCP_OPENED, { ...UDP_OPENED, local: 2053 }]);
      await wrapper.get('[data-port-local="' + UDP_KEY + '"]').setValue("6000");
      await wrapper.get('[data-port-open="' + UDP_KEY + '"]').trigger("click");
      await flushPromises();
      expect(lastOpened()).toEqual([TCP_OPENED, { ...UDP_OPENED, local: 6000 }]);
    });

    it("the TCP row of the same number is not affected", () => {
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053) }), udpRow({ opened: failed })], [TCP_OPENED]);
      expect(rowOf(wrapper, TCP_KEY).text()).not.toContain("is busy");
    });
  });
});

describe("the member's last error", () => {
  const TEXTS = [
    ["unsupported", "The member runs an old version: UDP is not supported"],
    ["offline", "The member is offline"],
    ["not_opened", "The member has not shared this port with you"],
    ["no_service", "Nothing is listening on the member's port"],
    ["timed_out", "The member did not answer in time"],
    ["failed", "Couldn't reach the member's port"],
  ];

  describe("on an opened UDP row", () => {
    for (const [code, text] of TEXTS) {
      it("last_error " + code + " shows: " + text, () => {
        const { wrapper } = mountNet([udpRow({ opened: listening(5353, code) })], [UDP_OPENED]);
        const line = lastErrorOf(wrapper, UDP_KEY);
        expect(line.text()).toBe(text);
        expect(line.classes()).toContain("field-error-text");
      });
    }

    it("last_error null shows no line", () => {
      const { wrapper } = mountNet([udpRow({ opened: listening(5353, null) })], [UDP_OPENED]);
      expect(lastErrorOf(wrapper, UDP_KEY).exists()).toBe(false);
    });

    it("the line is still there in the \"Change number\" view", async () => {
      const { wrapper } = mountNet([udpRow({ opened: listening(5353, "offline") })], [UDP_OPENED]);
      await wrapper.get('[data-port-change="' + UDP_KEY + '"]').trigger("click");
      expect(wrapper.find('[data-port-local="' + UDP_KEY + '"]').exists()).toBe(true);
      expect(lastErrorOf(wrapper, UDP_KEY).text()).toBe("The member is offline");
    });

    it("the line goes away when the next snapshot has no error", async () => {
      const { wrapper, store } = mountNet([udpRow({ opened: listening(5353, "no_service") })], [UDP_OPENED]);
      expect(lastErrorOf(wrapper, UDP_KEY).exists()).toBe(true);
      applySnapshot(store, { state: baseState({ ports: { published: [], network: [udpRow({ opened: listening(5353, null) })] } }) });
      await flushPromises();
      expect(lastErrorOf(wrapper, UDP_KEY).exists()).toBe(false);
    });

    it("the line changes to the text of the new error", async () => {
      const { wrapper, store } = mountNet([udpRow({ opened: listening(5353, "no_service") })], [UDP_OPENED]);
      applySnapshot(store, { state: baseState({ ports: { published: [], network: [udpRow({ opened: listening(5353, "timed_out") })] } }) });
      await flushPromises();
      expect(lastErrorOf(wrapper, UDP_KEY).text()).toBe("The member did not answer in time");
    });
  });

  describe("on a UDP row that is not opened", () => {
    it("shows no line", () => {
      const { wrapper } = mountNet([udpRow()]);
      expect(lastErrorOf(wrapper, UDP_KEY).exists()).toBe(false);
    });
  });

  describe("on an opened TCP row", () => {
    it("unsupported reads as \"Couldn't reach the member's port\"", () => {
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053, "unsupported") })], [TCP_OPENED]);
      expect(lastErrorOf(wrapper, TCP_KEY).text()).toBe("Couldn't reach the member's port");
    });

    it("no_service shows its text under the TCP key", () => {
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053, "no_service") })], [TCP_OPENED]);
      expect(lastErrorOf(wrapper, TCP_KEY).text()).toBe("Nothing is listening on the member's port");
    });

    it("last_error null shows no line", () => {
      const { wrapper } = mountNet([tcpRow({ opened: listening(2053, null) })], [TCP_OPENED]);
      expect(lastErrorOf(wrapper, TCP_KEY).exists()).toBe(false);
    });
  });

  describe("on both rows of one number", () => {
    it("each line sits in its own row", () => {
      const { wrapper } = mountNet(
        [tcpRow({ opened: listening(2053, "timed_out") }), udpRow({ opened: listening(5353, "offline") })],
        [TCP_OPENED, UDP_OPENED],
      );
      expect(rowOf(wrapper, TCP_KEY).get('[data-port-last-error="' + TCP_KEY + '"]').text()).toBe("The member did not answer in time");
      expect(rowOf(wrapper, UDP_KEY).get('[data-port-last-error="' + UDP_KEY + '"]').text()).toBe("The member is offline");
    });
  });
});
