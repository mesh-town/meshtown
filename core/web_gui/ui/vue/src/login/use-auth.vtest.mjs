/* `useAuth.noPassword`: true only when the status is received and `password_set === false`. */

import { describe, expect, it } from "vitest";
import { defineComponent } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { useAuth } from "./use-auth.ts";

function mountAuth(authStatus) {
  let auth;
  const comp = defineComponent({ setup() { auth = useAuth({ awaitingToken: false }, { authStatus, getToken: () => "" }); return () => null; } });
  mount(comp);
  return auth;
}

const st = (password_set) => ({ password_set, access: "localnet", listening: "localnet", via: null });

describe("useAuth.noPassword", () => {
  it("password_set:false — true", async () => {
    const auth = mountAuth(async () => st(false));
    await flushPromises();
    expect(auth.noPassword.value).toBe(true);
  });

  it("password_set:true — false", async () => {
    const auth = mountAuth(async () => st(true));
    await flushPromises();
    expect(auth.noPassword.value).toBe(false);
  });

  it("status not received — false", async () => {
    const auth = mountAuth(async () => { throw new Error("down"); });
    await flushPromises();
    expect(auth.noPassword.value).toBe(false);
  });
});
