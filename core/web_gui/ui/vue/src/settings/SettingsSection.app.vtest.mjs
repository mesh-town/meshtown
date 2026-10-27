/* `SettingsSection.vue` inside the Android app (the user agent carries `MeshTownApp/`): under "Stop Mesh Town" the note tells
 * to open the app, not to use the tray icon; the Leave hint says "phone". On a desktop both keep their old words. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { STOPPED_NOTE, STOPPED_NOTE_APP } from "../common/pac-texts.ts";
import { api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import SettingsSection from "./SettingsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
const NOTE_APP = "While Mesh Town is stopped, sites in the browser open directly, without an exit; in strict mode they don't open at all (local addresses do). After it starts, everything comes back in about 15 seconds. To start it again, open the Mesh Town app.";
let mounted;

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("Settings in the app", () => {
  it("the note under the stop button is STOPPED_NOTE_APP: open the Mesh Town app", () => {
    const { wrapper } = setup(APP_UA);
    const note = wrapper.get("#stop-node-btn").element.nextElementSibling.textContent.trim();
    expect(note).toBe(NOTE_APP);
    expect(note).toBe(STOPPED_NOTE_APP);
  });

  it("the note does not mention the icon or the clock", () => {
    const { wrapper } = setup(APP_UA);
    const note = wrapper.get("#stop-node-btn").element.nextElementSibling.textContent;
    expect(note).not.toMatch(/icon|clock/);
  });

  it("the Leave hint says this phone", () => {
    const { wrapper } = setup(APP_UA);
    expect(wrapper.get("#settings-leave-row .hint").text()).toBe("Disconnect this phone from \"demo-network\" for good. You'll need a new invite to come back.");
  });
});

describe("Settings on a desktop", () => {
  it("the note under the stop button is STOPPED_NOTE, with the icon near the clock", () => {
    const { wrapper } = setup(DESKTOP_UA);
    const note = wrapper.get("#stop-node-btn").element.nextElementSibling.textContent.trim();
    expect(note).toBe(STOPPED_NOTE);
    expect(note).toContain("icon near the clock");
  });

  it("the Leave hint says this computer", () => {
    const { wrapper } = setup(DESKTOP_UA);
    expect(wrapper.get("#settings-leave-row .hint").text()).toBe("Disconnect this computer from \"demo-network\" for good. You'll need a new invite to come back.");
  });
});

function setup(userAgent) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  api.mockResolvedValue({});
  const config = baseConfig({ socks: { enabled: true, port: 1080 } });
  const network = { id_hex: "net-aaaa", label: "demo-network", masked: "demo-network--••••" };
  return (mounted = mountSection(SettingsSection, { config, configMeta: META, configFile: { writable: true }, state: baseState({ network }) }));
}
