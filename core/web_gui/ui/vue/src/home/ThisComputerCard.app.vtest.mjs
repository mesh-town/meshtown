/* `home/ThisComputerCard.vue` inside the Android app (the user agent carries `MeshTownApp/`): the card is titled "This phone";
 * on a desktop the title stays "This computer". */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { baseState, mountSection } from "../testing/section.mjs";
import ThisComputerCard from "./ThisComputerCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });

beforeEach(() => localStorage.clear());
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("ThisComputerCard in the app", () => {
  it("the title and the whole card have no word \"computer\"", () => {
    setUa("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android");
    mounted = mountSection(ThisComputerCard, { state: baseState() });
    expect(mounted.wrapper.get("#home-computer-title").text()).toBe("This phone");
    expect(mounted.wrapper.get("#home-computer").text()).not.toContain("computer");
  });

  it("on a desktop the title is \"This computer\"", () => {
    mounted = mountSection(ThisComputerCard, { state: baseState() });
    expect(mounted.wrapper.get("#home-computer-title").text()).toBe("This computer");
  });
});
