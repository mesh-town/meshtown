/* The "update available" banner: a newer release on GitHub or at a member, equal version, Dismiss, a failed request. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { baseState, mountSection, OWN_VERSION, OLD_VERSION } from "../testing/section.mjs";
import UpdateBanner from "./UpdateBanner.vue";

let mounted;

function release(tag) {
  return vi.fn(async () => ({ ok: true, json: async () => ({ tag_name: tag }) }));
}

async function mountBanner(fetchStub, state = baseState()) {
  vi.stubGlobal("fetch", fetchStub);
  mounted = mountSection(UpdateBanner, { state });
  await flushPromises();
  return mounted.wrapper;
}

function peersWith(version) {
  return baseState().peers.map((p) => (p.self ? p : { ...p, version }));
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  mounted.wrapper.unmount();
  vi.unstubAllGlobals();
});

describe("UpdateBanner", () => {
  it("shows the banner when GitHub has a newer release", async () => {
    const wrapper = await mountBanner(release("v9.0.0"));
    expect(wrapper.get("#update-banner").text()).toContain(`Mesh Town 9.0.0 is available (this node runs ${OWN_VERSION}).`);
    const link = wrapper.get("#update-download");
    expect(link.attributes("href")).toBe("https://github.com/mesh-town/meshtown-dist#readme");
    expect(link.attributes("rel")).toBe("noopener");
    expect(wrapper.get("#update-hint").text()).toContain("brew upgrade meshtown");
  });

  it("shows the banner when a member runs a newer version", async () => {
    const wrapper = await mountBanner(release(OWN_VERSION), baseState({ peers: peersWith("7.1.0") }));
    expect(wrapper.get("#update-banner").text()).toContain("Mesh Town 7.1.0 is available");
  });

  it("stays hidden when versions are equal or older", async () => {
    const wrapper = await mountBanner(release(OWN_VERSION), baseState({ peers: peersWith(OLD_VERSION) }));
    expect(wrapper.find("#update-banner").exists()).toBe(false);
  });

  it("Dismiss hides the banner and remembers the version", async () => {
    const wrapper = await mountBanner(release("9.0.0"));
    await wrapper.get("#update-dismiss").trigger("click");
    expect(wrapper.find("#update-banner").exists()).toBe(false);
    mounted.wrapper.unmount();
    expect((await mountBanner(release("9.0.0"))).find("#update-banner").exists()).toBe(false);
    mounted.wrapper.unmount();
    expect((await mountBanner(release("9.0.1"))).find("#update-banner").exists()).toBe(true);
  });

  it("a failed request leaves no banner and no error", async () => {
    const wrapper = await mountBanner(vi.fn(async () => { throw new Error("offline"); }));
    expect(wrapper.find("#update-banner").exists()).toBe(false);
    mounted.wrapper.unmount();
    const bad = await mountBanner(vi.fn(async () => ({ ok: false, json: async () => ({}) })));
    expect(bad.find("#update-banner").exists()).toBe(false);
  });

  it("on Android links to the app releases and hides the package hint", async () => {
    const ua = vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Linux; Android 14)");
    const wrapper = await mountBanner(release("9.0.0"));
    expect(wrapper.get("#update-download").attributes("href")).toBe("https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk");
    expect(wrapper.find("#update-hint").exists()).toBe(false);
    ua.mockRestore();
  });
});
