/* The version source of the update check: the latest release of the distribution repository, with a fallback to peers. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, ref } from "vue";
import { baseState, OWN_VERSION } from "../testing/section.mjs";
import { useUpdateCheck } from "./use-update-check.ts";

const DIST_LATEST = "https://api.github.com/repos/mesh-town/meshtown-dist/releases/latest";
let wrapper;

function mountCheck(state) {
  const Probe = defineComponent({
    setup() {
      return useUpdateCheck(ref(state));
    },
    template: "<div>{{ newer }}</div>",
  });
  wrapper = mount(Probe);
  return wrapper;
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  wrapper.unmount();
  vi.unstubAllGlobals();
});

describe("useUpdateCheck source", () => {
  it("asks the distribution repository for the latest release", async () => {
    const fetchStub = vi.fn(async () => ({ ok: true, json: async () => ({ tag_name: "v9.0.0" }) }));
    vi.stubGlobal("fetch", fetchStub);
    mountCheck(baseState());
    await flushPromises();
    expect(fetchStub).toHaveBeenCalledTimes(1);
    expect(fetchStub.mock.calls[0][0]).toBe(DIST_LATEST);
    expect(wrapper.text()).toBe("9.0.0");
  });

  it("falls back to the version a peer announces when the lookup answers 404", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: false, status: 404, json: async () => ({}) })));
    const state = baseState();
    state.peers = state.peers.map((p) => (p.self ? p : { ...p, version: "9.1.0" }));
    mountCheck(state);
    await flushPromises();
    expect(OWN_VERSION).not.toBe("9.1.0");
    expect(wrapper.text()).toBe("9.1.0");
  });
});
