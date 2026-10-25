/* "Settings": a click on "Save" right after the node name was saved by blur is not a conflict. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { ApiError } from "../core/api.ts";
import { demoConfig } from "../testing/demo-node.mjs";
import { mountSection } from "../testing/section.mjs";
import SettingsSection from "./SettingsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
const NAME = "#settings-main-card #settings-node-name";
const PORT = 'input[aria-label="Proxy port (SOCKS5 and HTTP)"]';
let mounted;

afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
});

// A node that answers 409 to any revision but the current one and replies to a write after a delay.
function slowNode() {
  const server = { config: null, rev: 1 };
  const puts = [];
  api.mockImplementation(async (_method, _path, body) => {
    puts.push(body.rev);
    if (body.rev !== server.rev) throw new ApiError(409, "stale_rev", "stale", { rev: server.rev });
    server.config = structuredClone(body.config);
    server.rev += 1;
    const reply = { config: structuredClone(server.config), rev: server.rev };
    await new Promise((r) => setTimeout(r, 20));
    return reply;
  });
  return { server, puts };
}

describe("SettingsSection: name blur, then Save", () => {
  it("both writes succeed, no conflict block, the name is saved", async () => {
    const node = slowNode();
    mounted = mountSection(SettingsSection, { config: demoConfig(), configMeta: META, configFile: { writable: true } });
    const { wrapper } = mounted;
    await wrapper.get(PORT).setValue("1081");
    await wrapper.get(NAME).setValue("office");
    await wrapper.get(NAME).trigger("blur");
    await wrapper.get("#settings-save-btn").trigger("click");
    await new Promise((r) => setTimeout(r, 120));
    await flushPromises();
    expect(wrapper.find("#settings-conflict-slot [id^='settings-conflict']").exists()).toBe(false);
    expect(wrapper.find("#settings-conflict-overwrite-btn").exists()).toBe(false);
    expect(node.puts).toEqual([1, 2]);
    expect(node.server.config.node.name).toBe("office");
    expect(wrapper.get(NAME).element.value).toBe("office");
  });
});
