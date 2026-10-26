/* The network key card lives in the "Details" of the "Diagnostics" tab: we mount the whole section. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import DiagnosticsSection from "./DiagnosticsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const FULL = "mesh2-FULLKEYFULLKEY";
let mounted;
const mountDiag = () => (mounted = mountSection(DiagnosticsSection));
const keyText = (w) => w.get("#netkey").text();
const keyBtn = (w) => w.get("#netkey-show-btn");

afterEach(() => mounted.wrapper.unmount());

describe("NetKeyCard", () => {
  it("before the click — the network mask and the \"Show\" button", () => {
    const { wrapper } = mountDiag();
    expect(keyText(wrapper)).toBe("mesh2-••••-••••");
    expect(keyBtn(wrapper).text()).toBe("Show");
  });

  it("the \"Network key\" title, h3 \"Connection status\" above the table", () => {
    const { wrapper } = mountDiag();
    expect(wrapper.get("#net-key-title").text()).toBe("Mesh Town network key");
    expect(wrapper.get("#diag-specialist h3").text()).toBe("Connection status");
  });

  it("click — GET /api/network/key, the full key and \"Hide\"; the second click hides it", async () => {
    api.mockResolvedValue({ key: FULL });
    const { wrapper } = mountDiag();
    await keyBtn(wrapper).trigger("click");
    await flushPromises();
    expect(api).toHaveBeenCalledWith("GET", "/api/network/key");
    expect(keyText(wrapper)).toBe(FULL);
    expect(keyBtn(wrapper).text()).toBe("Hide");
    await keyBtn(wrapper).trigger("click");
    expect(keyText(wrapper)).toBe("mesh2-••••-••••");
    expect(keyBtn(wrapper).text()).toBe("Show");
  });

  it("api refusal — role=status with text, the key stays under the mask", async () => {
    api.mockRejectedValue(new ApiError(500, "http_500", "The node responded with an error 500"));
    const { wrapper } = mountDiag();
    await keyBtn(wrapper).trigger("click");
    await flushPromises();
    expect(wrapper.get("#net-key-card [role=status]").text()).toBe("Couldn't get the network key: the node responded with an error 500");
    expect(keyText(wrapper)).toBe("mesh2-••••-••••");
  });

  it("network.id_hex changes in the snapshot — the key is under the mask again", async () => {
    api.mockResolvedValue({ key: FULL });
    const { wrapper, store } = mountDiag();
    await keyBtn(wrapper).trigger("click");
    await flushPromises();
    expect(keyText(wrapper)).toBe(FULL);
    store.state = baseState({ network: { id_hex: "net-bbbb", masked: "mesh2-••••-2222" } });
    await flushPromises();
    expect(keyText(wrapper)).toBe("mesh2-••••-2222");
    expect(keyBtn(wrapper).text()).toBe("Show");
  });
});
