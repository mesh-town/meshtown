/* "This computer": the node name and "Allow others". */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { apiTable, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import ThisComputerCard from "./ThisComputerCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const putConfig = (body) => ({ config: body.config, rev: 2 });
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("ThisComputerCard", () => {
  it("the \"This computer\" region, the field is labeled \"Name of this computer\"", () => {
    const { wrapper } = mountComputer();
    expect(wrapper.get("#home-computer").attributes("role")).toBe("region");
    expect(wrapper.get('label[for="field-node-name"]').text()).toBe("Name of this computer");
    expect(wrapper.get("#field-node-name").element.value).toBe("finch-unwary");
  });

  it("Enter in the name field — PUT with the new name, the status is \"Saved\"", async () => {
    api.mockImplementation(apiTable({ "PUT /api/config": putConfig }));
    const { wrapper } = mountComputer();
    await wrapper.get("#field-node-name").setValue("office");
    await wrapper.get("#field-node-name").trigger("keydown", { key: "Enter" });
    await flushPromises();
    const body = api.mock.calls[0][2];
    expect(body.config.node.name).toBe("office");
    expect(wrapper.get("#field-node-name-status").text()).toBe("Saved");
  });

  it("the name is pinned by the command line — the field is unavailable", () => {
    const { wrapper } = mountComputer(baseState(), { configMeta: { read_only: ["node.name"], restart_fields: [], restart_pending: [], cli_overrides: [] } });
    expect(wrapper.get("#field-node-name").attributes("disabled")).toBeDefined();
  });

  it("the \"Allow others\" switch stands per the config and is labeled", () => {
    const { wrapper } = mountComputer(baseState(), { config: baseConfig({ exit: { enabled: true } }) });
    expect(wrapper.get("#home-exit-server").text()).toContain("Allow other members to open sites through this computer");
    expect(wrapper.get("#field-exit-enabled").element.checked).toBe(true);
  });

  it("the node refuses on toggling — the switch goes back", async () => {
    api.mockRejectedValue(new Error("network"));
    const { wrapper } = mountComputer(baseState(), { config: baseConfig({ exit: { enabled: true } }) });
    await wrapper.get("#field-exit-enabled").setValue(false);
    await flushPromises();
    expect(wrapper.get("#field-exit-enabled").element.checked).toBe(true);
  });
});

function mountComputer(state = baseState(), extra = {}) {
  return (mounted = mountSection(ThisComputerCard, { state, ...extra }));
}
