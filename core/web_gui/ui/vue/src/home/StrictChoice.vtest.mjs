/* The strict-mode question (shown under "More options" in the setup): marked by the node state, a click writes
 * `socks.strict`, a refusal shows an error and returns the radio to the state. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import StrictChoice from "./StrictChoice.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const proxy = (strict) => ({ socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac", ...(strict === undefined ? {} : { strict }) } });
let mounted;

afterEach(() => {
  mounted.wrapper.unmount();
  api.mockReset();
});

function mountChoice(strict, state = baseState(proxy(strict))) {
  return (mounted = mountSection(StrictChoice, { state, config: baseConfig({ socks: { enabled: true, port: 1080 } }) }));
}

const checked = (wrapper) => [wrapper.get("#strict-off").element.checked, wrapper.get("#strict-on").element.checked];

describe("StrictChoice", () => {
  it("the question and two answers", () => {
    const { wrapper } = mountChoice(false);
    expect(wrapper.get("#strict-q").text()).toBe("If Mesh Town isn't running:");
    expect(wrapper.get("#strict-off").element.parentElement.textContent).toBe("Open sites directly (recommended)");
    expect(wrapper.get("#strict-on").element.parentElement.textContent).toBe("Don't open sites — for services that refuse sign-ins from another country");
  });

  it.each([[false, [true, false]], [true, [false, true]], [undefined, [true, false]]])("state strict=%s: radios %j", (strict, expected) => {
    expect(checked(mountChoice(strict).wrapper)).toEqual(expected);
  });

  it("the root is a fieldset with the legend, not a collapsible block", () => {
    const { wrapper } = mountChoice(false);
    expect(wrapper.get("#strict-choice").element.tagName).toBe("FIELDSET");
    expect(wrapper.find("details").exists()).toBe(false);
    expect(wrapper.get("#strict-choice > legend").attributes("id")).toBe("strict-q");
  });

  it("proxy off: hidden", () => {
    const { wrapper } = mountChoice(false, baseState({ socks: { enabled: false, listen: null, pac_url: null } }));
    expect(wrapper.find("#strict-choice").exists()).toBe(false);
  });

  it("a click on the second answer sends {socks.strict: true}", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountChoice(false);
    await wrapper.get("#strict-on").setValue(true);
    await flushPromises();
    const [method, path, body] = api.mock.calls.find((c) => c[0] === "PUT");
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.socks.strict).toBe(true);
    expect(wrapper.find("#strict-error").exists()).toBe(false);
    expect(checked(wrapper)).toEqual([false, true]);
  });

  it("a refusal: the error line, the radio goes back to the state", async () => {
    api.mockRejectedValue(new ApiError(500, "write_failed", "disk full"));
    const { wrapper } = mountChoice(false);
    await wrapper.get("#strict-on").setValue(true);
    await flushPromises();
    expect(wrapper.get("#strict-error").text()).not.toBe("");
    expect(checked(wrapper)).toEqual([true, false]);
  });

  it("a refusal, then a successful retry: the error goes away, the radio is on the new value", async () => {
    api.mockRejectedValueOnce(new ApiError(500, "write_failed", "disk full"));
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountChoice(false);
    await wrapper.get("#strict-on").setValue(true);
    await flushPromises();
    expect(wrapper.find("#strict-error").exists()).toBe(true);
    await wrapper.get("#strict-on").setValue(true);
    await flushPromises();
    expect(wrapper.find("#strict-error").exists()).toBe(false);
    expect(checked(wrapper)).toEqual([false, true]);
  });
});
