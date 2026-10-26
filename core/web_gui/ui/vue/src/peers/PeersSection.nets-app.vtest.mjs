/* The hint about several networks inside the app (the user agent carries `MeshTownApp/`): it does not name the sites switch,
 * which the app does not have. A browser still reads the old wording. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });

beforeEach(() => localStorage.clear());
afterEach(() => {
  unmountPage();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("the hint of two networks", () => {
  it("in the app the hint leaves out the sites switch", async () => {
    setUa(APP_UA);
    const { wrapper } = await mountPage(createFakeNode(twoNets()), { hash: "peers" });
    const text = wrapper.get("#peers-nets-hint").text();
    expect(text).toContain("You’re now in 2 networks.");
    expect(text).toContain('Your name and "Leave network" are now in each network’s tab.');
    expect(text).not.toContain("Let members open sites");
  });

  it("in a browser the hint is the one it always was", async () => {
    const { wrapper } = await mountPage(createFakeNode(twoNets()), { hash: "peers" });
    expect(wrapper.get("#peers-nets-hint").text()).toContain('Your name, "Let members open sites" and "Leave network"');
  });
});
