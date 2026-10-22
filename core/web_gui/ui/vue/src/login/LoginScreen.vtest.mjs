/* Sign-in screen: password → `POST /api/auth/login`; success — the `done` event, errors 401/409/429 — as text. */

import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import LoginScreen from "./LoginScreen.vue";

function stubLogin(status, body, headers = {}) {
  const fetch = vi.fn(async () => new Response(JSON.stringify(body), { status, headers }));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

async function submit(w, password = "correct-password") {
  await w.get("#login-password").setValue(password);
  await w.get("#login-form").trigger("submit");
  await flushPromises();
}

describe("LoginScreen", () => {
  it("success: one request with the password and the done event", async () => {
    const fetch = stubLogin(200, { via: "password" });
    const w = mount(LoginScreen);
    await submit(w);
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][0]).toBe("/api/auth/login");
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({ password: "correct-password" });
    expect(w.emitted("done")).toHaveLength(1);
    expect(w.find("#login-error").exists()).toBe(false);
  });

  it("401: \"Wrong password\", no done event, field cleared", async () => {
    stubLogin(401, { error: "bad_password", message: "x" });
    const w = mount(LoginScreen);
    await submit(w);
    expect(w.get("#login-error").text()).toContain("Wrong password");
    expect(w.emitted("done")).toBeUndefined();
    expect(w.get("#login-password").element.value).toBe("");
  });

  it("429: how many seconds to wait — from Retry-After", async () => {
    stubLogin(429, { error: "too_many_attempts", message: "x" }, { "Retry-After": "37" });
    const w = mount(LoginScreen);
    await submit(w);
    expect(w.get("#login-error").text()).toContain("37 s");
    expect(w.emitted("done")).toBeUndefined();
  });

  it("409: the form is replaced by the \"Password not set yet\" instructions", async () => {
    stubLogin(409, { error: "no_password", message: "x" });
    const w = mount(LoginScreen);
    await submit(w);
    expect(w.find("#login-form").exists()).toBe(false);
    expect(w.get("h2").text()).toBe("Password not set yet");
    expect(w.get("#no-password-help").text()).toContain("meshtown password reset");
  });

  it("no-password prop: instructions right away, no form, steps in a list", () => {
    const w = mount(LoginScreen, { props: { noPassword: true } });
    expect(w.find("#login-form").exists()).toBe(false);
    expect(w.text()).not.toContain("The node page is protected by a password");
    expect(w.findAll("ol li").length).toBe(3);
  });

  it("empty password: the \"Sign in\" button is disabled, no request", async () => {
    const fetch = stubLogin(200, { via: "password" });
    const w = mount(LoginScreen);
    expect(w.get("#login-submit").attributes("disabled")).toBeDefined();
    await w.get("#login-form").trigger("submit");
    expect(fetch).not.toHaveBeenCalled();
  });

  it("password field is type=password, without autofill by others", async () => {
    const w = mount(LoginScreen);
    expect(w.get("#login-password").attributes("type")).toBe("password");
    expect(w.get("#login-password").attributes("autocomplete")).toBe("current-password");
  });
});
