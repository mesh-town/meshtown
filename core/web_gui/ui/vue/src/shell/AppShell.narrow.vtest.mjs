/* The page skeleton on a narrow screen: the "Site checks" section is called "Sites" in the tab and in its heading. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { stubNarrow } from "../testing/narrow-media.mjs";
import AppShell from "./AppShell.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("fetch", async () => new Response("{}", { status: 404 }));
  sessionStorage.clear();
  location.hash = "";
  api.mockImplementation(async (_method, path) => {
    if (path === "/api/state") return baseState({ config_rev: 1 });
    if (path === "/api/config") return { config: baseConfig(), rev: 1 };
    return null;
  });
});
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
  location.hash = "";
});

describe("AppShell on a narrow screen", () => {
  it("the section heading says \"Sites\", the others keep their names", async () => {
    const { wrapper } = await mountShell(true);
    expect(wrapper.findAll(".tab-title").map((t) => t.text())).toEqual(["Networks", "Sites", "Diagnostics", "Log", "Settings"]);
  });

  it("the tab says \"Sites\" and labels its panel", async () => {
    const { wrapper } = await mountShell(true);
    expect(wrapper.get("#tab-domains").text()).toBe("Sites");
    const panel = wrapper.get("#sec-domains");
    expect(wrapper.get("#" + panel.attributes("aria-labelledby")).text()).toBe("Sites");
  });

  it("a section under \"More\" is still a tabpanel, hidden until chosen", async () => {
    const { wrapper } = await mountShell(true);
    const panel = wrapper.get("#sec-log");
    expect(panel.attributes("role")).toBe("tabpanel");
    expect(panel.attributes("hidden")).toBeDefined();
  });
});

describe("AppShell on a wide screen", () => {
  it("the heading keeps the long name \"Site checks\"", async () => {
    const { wrapper } = await mountShell(false);
    expect(wrapper.findAll(".tab-title").map((t) => t.text())).toContain("Site checks");
    expect(wrapper.get("#tab-domains").text()).toBe("Site checks");
  });
});

async function mountShell(narrow) {
  stubNarrow(narrow);
  mounted = mountSection(AppShell, { configMeta: { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] } });
  await flushPromises();
  return mounted;
}
