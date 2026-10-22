/* Password row in "Settings": label by `password_set`, change without the old password, format error, "Sign out". */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { TOKEN_KEY } from "../core/token.ts";
import { AuthError, authStatus, logout, setPassword } from "./login-api.ts";
import PasswordRow from "./PasswordRow.vue";

vi.mock("./login-api.ts", async (orig) => ({ ...(await orig()), authStatus: vi.fn(), setPassword: vi.fn(), logout: vi.fn() }));

const status = (over = {}) => ({ password_set: true, access: "localnet", listening: "localnet", via: "password", ...over });

async function mountRow(over = {}) {
  authStatus.mockResolvedValue(status(over));
  const w = mount(PasswordRow, { props: { revision: 1 } });
  await flushPromises();
  return w;
}

beforeEach(() => vi.resetAllMocks());
afterEach(() => sessionStorage.clear());

describe("PasswordRow", () => {
  it("password not set — the \"Set password\" button", async () => {
    const w = await mountRow({ password_set: false });
    expect(w.get("#password-save-btn").text()).toBe("Set password");
  });

  it("password set — the \"Change password\" button, the old password is not asked", async () => {
    const w = await mountRow();
    expect(w.get("#password-save-btn").text()).toBe("Change password");
    expect(w.findAll('input[type="password"]')).toHaveLength(1);
  });

  it("successful change: setPassword with the new password, a message, field cleared", async () => {
    setPassword.mockResolvedValue({ password_set: true });
    const w = await mountRow();
    await w.get("#password-new").setValue("new-password-1");
    await w.get("#password-save-btn").trigger("click");
    await flushPromises();
    expect(setPassword).toHaveBeenCalledWith("new-password-1");
    expect(w.get("#password-message").text()).toContain("Password saved");
    expect(w.get("#password-new").element.value).toBe("");
  });

  it("400 bad_password_format — text from the response", async () => {
    setPassword.mockRejectedValue(new AuthError(400, "bad_password_format", "The password must be at least 8 characters"));
    const w = await mountRow();
    await w.get("#password-new").setValue("short");
    await w.get("#password-save-btn").trigger("click");
    await flushPromises();
    expect(w.get("#password-message").text()).toContain("The password must be at least 8 characters");
  });

  it("\"Sign out\": calls logout, clears the token, reports upward", async () => {
    logout.mockResolvedValue({});
    sessionStorage.setItem(TOKEN_KEY, "tok");
    const w = await mountRow();
    await w.get("#logout-btn").trigger("click");
    await flushPromises();
    expect(logout).toHaveBeenCalledTimes(1);
    expect(sessionStorage.getItem(TOKEN_KEY)).toBeNull();
    expect(w.emitted("loggedout")).toHaveLength(1);
  });

  it("access ≠ listening — a restart hint; equal — without it", async () => {
    const w = await mountRow({ access: "localnet", listening: "localhost" });
    expect(w.text()).toContain("will take effect after the node restarts");
    const w2 = await mountRow();
    expect(w2.text()).not.toContain("will take effect after the node restarts");
  });
});
