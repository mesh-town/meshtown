/* First-password screen: the password twice and the access choice; success — `PUT /api/auth/password`, then `PUT /api/config`. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { STORE_KEY, createStore } from "../core/store.ts";
import { baseConfig } from "../testing/section.mjs";
import SetupScreen from "./SetupScreen.vue";

const calls = [];

function stubNode({ passwordStatus = 200 } = {}) {
  calls.length = 0;
  vi.stubGlobal("fetch", vi.fn(async (url, init) => {
    calls.push({ method: init.method, url, body: init.body ? JSON.parse(init.body) : null });
    if (url === "/api/auth/password") return new Response(JSON.stringify(passwordStatus === 200 ? { password_set: true } : { error: "bad_password_format", message: "too short" }), { status: passwordStatus });
    return new Response(JSON.stringify({ rev: 2, config: { ...baseConfig(), web: { access: "localhost" } }, file: null }), { status: 200 });
  }));
}

function mountSetup() {
  const store = createStore();
  store.config = baseConfig();
  store.configRev = 1;
  return mount(SetupScreen, { global: { provide: { [STORE_KEY]: store } } });
}

async function fill(w, one, two) {
  await w.get("#setup-password").setValue(one);
  await w.get("#setup-password2").setValue(two);
}

afterEach(() => { calls.length = 0; });

describe("SetupScreen", () => {
  it("passwords don't match: button disabled, a hint, no requests", async () => {
    stubNode();
    const w = mountSetup();
    await fill(w, "first-password", "second-password");
    expect(w.get("#setup-submit").attributes("disabled")).toBeDefined();
    expect(w.text()).toContain("Passwords don't match");
    await w.get("#setup-form").trigger("submit");
    expect(calls).toEqual([]);
  });

  it("shorter than 8 characters: button disabled", async () => {
    stubNode();
    const w = mountSetup();
    await fill(w, "short", "short");
    expect(w.get("#setup-submit").attributes("disabled")).toBeDefined();
  });

  it("access choice explanation: only about access to the Mesh Town settings, not to the network and traffic", () => {
    stubNode();
    const w = mountSetup();
    expect(w.text()).toContain("this only concerns access to the Mesh Town settings, not to the network or traffic");
    const values = w.findAll("#setup-access option").map((o) => o.attributes("value"));
    expect(values).toEqual(["localnet", "localhost"]);
  });

  it("success: two requests in order — password, then PUT /api/config with web.access; the done event", async () => {
    stubNode();
    const w = mountSetup();
    await fill(w, "good-password", "good-password");
    await w.get("#setup-access").setValue("localhost");
    await w.get("#setup-form").trigger("submit");
    await flushPromises();
    expect(calls.map((c) => c.method + " " + c.url)).toEqual(["PUT /api/auth/password", "PUT /api/config"]);
    expect(calls[0].body).toEqual({ password: "good-password" });
    expect(calls[1].body.config.web.access).toBe("localhost");
    expect(w.emitted("done")).toHaveLength(1);
  });

  it("the node rejected the password: error text, the config was not reached, no done", async () => {
    stubNode({ passwordStatus: 400 });
    const w = mountSetup();
    await fill(w, "good-password", "good-password");
    await w.get("#setup-form").trigger("submit");
    await flushPromises();
    expect(calls.map((c) => c.url)).toEqual(["/api/auth/password"]);
    expect(w.get("#setup-error").text()).toContain("too short");
    expect(w.emitted("done")).toBeUndefined();
  });
});
