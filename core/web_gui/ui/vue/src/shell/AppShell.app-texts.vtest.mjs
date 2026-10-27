/* The whole page inside the Android app: no tab shows the word "computer" or a pointer to the icon near the clock.
 * The same scenarios in a browser keep their wording. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ANNA_FULL, ME_FULL, managedNode } from "../testing/managed-fixtures.mjs";
import { mountPage, polls, unmountPage } from "../testing/page.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android";
const BROWSER_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });
const FORBIDDEN = /\bcomputer\b|near the clock|tray icon/i;

beforeEach(() => localStorage.clear());
afterEach(() => {
  unmountPage();
  Reflect.deleteProperty(navigator, "userAgent");
});

const SCREENS = [
  ["Home", ""],
  ["Members", "peers"],
  ["Settings", "settings"],
];

describe("the page in the app", () => {
  describe.each(SCREENS)("%s", (_name, hash) => {
    it("the only admin: no computer, no icon near the clock", async () => {
      setUa(APP_UA);
      const { wrapper } = await mountPage(managedNode({ meRole: "admin", admins: [ME_FULL] }), { hash });
      expect(wrapper.element.textContent).not.toMatch(FORBIDDEN);
    });

  });

  describe.each(SCREENS.slice(1))("%s in a browser", (_name, hash) => {
    it("the same page keeps the word computer", async () => {
      setUa(BROWSER_UA);
      const { wrapper } = await mountPage(managedNode({ meRole: "admin", admins: [ME_FULL] }), { hash });
      expect(wrapper.element.textContent).toMatch(/\bcomputer\b/);
    });
  });

  it("Members: the second-admin hint says phone", async () => {
    setUa(APP_UA);
    const { wrapper } = await mountPage(managedNode({ meRole: "admin", admins: [ME_FULL] }), { hash: "peers" });
    expect(wrapper.get("#second-admin-hint").text()).toContain("if this phone is lost");
  });

  it("Members: a plain member sees no computer", async () => {
    setUa(APP_UA);
    const { wrapper } = await mountPage(managedNode({ meRole: "member", admins: [ANNA_FULL] }), { hash: "peers" });
    expect(wrapper.element.textContent).not.toMatch(FORBIDDEN);
  });

  it("the strip of an old link says to open the app", async () => {
    setUa(APP_UA);
    const node = managedNode();
    node.routes["GET /api/overview"] = { status: 401, body: { error: "bad_token" } };
    node.routes["GET /api/state"] = { status: 401, body: { error: "bad_token" } };
    const { wrapper } = await mountPage(node, { ready: false });
    await polls(node, 1).catch(() => {});
    expect(wrapper.element.textContent).not.toMatch(FORBIDDEN);
  });

  it("the strip of a wrong address names no icon near the clock", async () => {
    setUa(APP_UA);
    const node = managedNode();
    node.routes["GET /api/overview"] = { status: 403, body: { error: "bad_origin", message: "wrong address" } };
    node.routes["GET /api/state"] = { status: 403, body: { error: "bad_origin", message: "wrong address" } };
    const { wrapper } = await mountPage(node, { ready: false });
    expect(wrapper.element.textContent).not.toMatch(FORBIDDEN);
    expect(wrapper.element.textContent).toContain("Mesh Town app");
  });
});
