/* An invite in the page address (`?invite=`): read once, kept for the forms, sent to "Add another network" when the node is in a network. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { api } from "../core/api.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { STATE } from "../shell/use-shell-support.mjs";
import InviteForm from "./InviteForm.vue";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INVITE = "mesh-invite2-abc";
const IN_NET = { ...STATE, network: { id_hex: "net-aaaa", masked: "mesh2-••••-••••" } };

// The address is read once per module: every test gets a fresh copy of the modules.
async function fresh() {
  vi.resetModules();
  const invite = await import("./use-invite-join.ts");
  const shell = await import("../shell/use-shell-support.mjs");
  return { invite, shell };
}

beforeEach(() => {
  api.mockReset();
  api.mockResolvedValue(null);
  history.replaceState(null, "", "/?invite=" + INVITE + "&x=1#home");
  location.hash = "";
});
afterEach(() => history.replaceState(null, "", "/"));

describe("the invite in the address", () => {
  it("read once, the address is cleaned, the string is held", async () => {
    const { invite } = await fresh();
    expect(invite.addressInvite()).toBe(INVITE);
    expect(location.search).toBe("?x=1");
    expect(invite.addressInvite()).toBe(INVITE);
  });

  it("a node without a network: the field of the form is filled, the address hash is not touched", async () => {
    const { invite, shell } = await fresh();
    const env = shell.build({});
    env.store.state = STATE;
    const form = invite.useInviteJoin(createStore(), { kind: "home" });
    expect(form.form.inviteInput).toBe(INVITE);
    expect(form.form.choice).toBe("join");
    expect(location.hash).toBe("");
    env.unmount();
  });

  it("a node in a network: the page goes to #peers/add and every new form gets the string", async () => {
    const { invite, shell } = await fresh();
    const env = shell.build({});
    env.store.state = IN_NET;
    expect(location.hash).toBe("#peers/add");
    expect(invite.useInviteJoin(createStore(), { kind: "add" }).form.inviteInput).toBe(INVITE);
    expect(invite.useInviteJoin(createStore(), { kind: "add" }).form.inviteInput).toBe(INVITE);
    env.unmount();
  });

  it("after the invite is spent a new form starts empty", async () => {
    const { invite } = await fresh();
    const api = async (method, path) => (path === "/api/network/join" ? { label: "family", restart: true, slot: "neta22" } : {});
    const first = invite.useInviteJoin(createStore(), { kind: "add" }, { api, storage: null, setTimer: () => 0 });
    expect(await first.submit()).toBe("joined");
    expect(invite.useInviteJoin(createStore(), { kind: "add" }).form.inviteInput).toBe("");
    expect(invite.addressInvite()).toBe("");
  });
});

describe("the preview of the invite from the address", () => {
  it("the invite from the address gets the preview and the note without a keystroke", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    api.mockImplementation(async (method, path) => {
      if (path !== "/api/network/inspect") throw new Error("unexpected request " + path);
      return { label: "family", id_hex: "aa", place: "here", slot: "neta22" };
    });
    const { invite } = await fresh();
    const store = createStore();
    store.nets = [{ id: "neta22", label: "family", id_hex: "aa", me_excluded: false }];
    const ids = { input: "x-input", button: "x-btn", error: "x-error", preview: "x-preview", note: "x-note", dup: "x-dup", open: "x-open" };
    const Host = defineComponent({
      setup() {
        const join = invite.useInviteJoin(store, { kind: "add" });
        return () => h(InviteForm, { join, ids });
      },
    });
    const wrapper = mount(Host, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
    expect(wrapper.get("#x-input").element.value).toBe(INVITE);
    expect(api).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
    await flushPromises();
    expect(api).toHaveBeenCalledWith("POST", "/api/network/inspect", { key: INVITE });
    expect(wrapper.find("#x-preview").exists()).toBe(true);
    expect(wrapper.get("#x-note").text()).toContain("You're already in");
    wrapper.unmount();
    vi.useRealTimers();
  });
});
