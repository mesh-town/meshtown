/* Shell and sign-in: 401 or "no session and no token" → sign-in screen; a session from a temporary token without a password →
 * first-password screen; the tray token never sees the screens. The whole page runs on a fake node. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const makeNode = (routes = {}) => createFakeNode({ state: demoState(), config: demoConfig(), routes });
const status = (over) => ({ status: 200, body: { password_set: true, access: "localnet", listening: "localnet", via: null, ...over } });
const DENY = { path: "/api/state", status: 401, body: { error: "bad_token", message: "x" } };

async function submitLogin(wrapper, password = "correct-password") {
  await wrapper.get("#login-password").setValue(password);
  await wrapper.get("#login-form").trigger("submit");
  await vi.advanceTimersByTimeAsync(0);
  await settle();
}

describe("sign-in screen in the shell", () => {
  it("401 on poll: the password field instead of the shell, no \"old link\" strip", async () => {
    const node = makeNode();
    node.fail = DENY;
    const { wrapper } = await mountPage(node, { ready: false });
    expect(wrapper.find("#login-form").exists()).toBe(true);
    expect(wrapper.find("#tabs").exists()).toBe(false);
    expect(wrapper.find("#token-url-input").exists()).toBe(false);
    expect(wrapper.get("#conn-strip").attributes("hidden")).toBeDefined();
  });

  it("successful sign-in: the screen disappears, polling resumes, the shell is ready", async () => {
    const node = makeNode({ "POST /api/auth/login": { status: 200, body: { via: "password" } } });
    node.fail = DENY;
    const { wrapper } = await mountPage(node, { ready: false });
    node.fail = null;
    await submitLogin(wrapper);
    await polls(node, 1);
    expect(node.count("POST", "/api/auth/login")).toBe(1);
    expect(wrapper.find("#login-form").exists()).toBe(false);
    expect(wrapper.find('[data-ready="true"]').exists()).toBe(true);
  });

  it("no session and no token (via: null, storage empty): sign-in screen right away", async () => {
    const node = makeNode();
    const { wrapper } = await mountPage(node, { token: "", ready: false });
    expect(wrapper.find("#login-form").exists()).toBe(true);
  });

  it("token in storage, via: null (header, as with the tray): no sign-in screen", async () => {
    const { wrapper } = await mountPage(makeNode());
    expect(wrapper.find("#login-form").exists()).toBe(false);
    expect(wrapper.find("#setup-form").exists()).toBe(false);
  });
});

describe("first-password screen in the shell", () => {
  it("via=temp and no password: the \"set a password\" screen instead of the shell", async () => {
    const node = makeNode({ "GET /api/auth/status": status({ via: "temp", password_set: false }) });
    const { wrapper } = await mountPage(node, { token: "", ready: false });
    expect(wrapper.find("#setup-form").exists()).toBe(true);
    expect(wrapper.find("#tabs").exists()).toBe(false);
  });

  it("via=temp, but the password is already set: no screen", async () => {
    const node = makeNode({ "GET /api/auth/status": status({ via: "temp", password_set: true }) });
    const { wrapper } = await mountPage(node, { token: "" });
    expect(wrapper.find("#setup-form").exists()).toBe(false);
  });

  it("via=token (permanent tray token) and no password: the page is not blocked", async () => {
    const node = makeNode({ "GET /api/auth/status": status({ via: "token", password_set: false }) });
    const { wrapper } = await mountPage(node);
    expect(wrapper.find("#setup-form").exists()).toBe(false);
    expect(wrapper.find("#login-form").exists()).toBe(false);
  });

  it("password set: the screen closes, the shell is in place", async () => {
    const node = makeNode({
      "GET /api/auth/status": status({ via: "temp", password_set: false }),
      "PUT /api/auth/password": { status: 200, body: { password_set: true } },
    });
    const { wrapper } = await mountPage(node, { token: "", ready: false });
    await wrapper.get("#setup-password").setValue("good-password");
    await wrapper.get("#setup-password2").setValue("good-password");
    await wrapper.get("#setup-form").trigger("submit");
    await vi.advanceTimersByTimeAsync(0);
    await settle();
    await polls(node, 1);
    expect(wrapper.find("#setup-form").exists()).toBe(false);
    expect(wrapper.find('[data-ready="true"]').exists()).toBe(true);
  });
});
