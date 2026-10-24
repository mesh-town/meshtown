/* The one invite form: ids from the place, the note under the field, "already in", a refusal, the answer for the place. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { ApiError, api } from "../core/api.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import InviteForm from "./InviteForm.vue";
import { useInviteJoin } from "./use-invite-join.ts";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INVITE = "mesh-invite2-abc";
const IDS = { input: "x-input", button: "x-btn", error: "x-error", preview: "x-preview", note: "x-note", dup: "x-dup", open: "x-open", status: "x-status", title: "x-title" };
const NETS = [{ id: "neta22", label: "family", id_hex: "aa", me_excluded: false }];

let wrapper;
let done;
let store;

function mountForm(ctx = { kind: "add" }) {
  store = createStore();
  store.nets = NETS;
  done = [];
  const Host = defineComponent({
    setup() {
      const join = useInviteJoin(store, ctx);
      return () => h(InviteForm, { join, ids: IDS, title: "A title", onDone: (...args) => done.push(args) });
    },
  });
  wrapper = mount(Host, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
}
const route = (table) => api.mockImplementation(async (method, path, body) => {
  const entry = table[path];
  if (entry === undefined) throw new Error("unexpected request " + path);
  return typeof entry === "function" ? entry(body) : entry;
});
const typeInvite = async () => {
  await wrapper.get("#x-input").setValue(INVITE);
  await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
  await flushPromises();
};
const posts = () => api.mock.calls.filter((c) => c[1] === "/api/network/join");

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  api.mockReset();
  history.replaceState(null, "", "/");
  location.hash = "";
});
afterEach(() => {
  wrapper.unmount();
  vi.useRealTimers();
});

describe("InviteForm", () => {
  it("the parts carry the ids of the place", () => {
    mountForm();
    for (const id of ["x-title", "x-input", "x-error", "x-btn"]) expect(wrapper.find("#" + id).exists(), id).toBe(true);
  });

  it("the note under the field comes 500 ms after typing, from the answer's place", async () => {
    route({ "/api/network/inspect": { label: "family", kind: "Peer", id_hex: "aa", place: "here", slot: "neta22" } });
    mountForm();
    await wrapper.get("#x-input").setValue(INVITE);
    expect(wrapper.find("#x-note").exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
    await flushPromises();
    expect(wrapper.get("#x-note").text()).toContain('You\'re already in "family".');
    expect(wrapper.get("#x-preview").text()).toContain("family");
  });

  it("\"already in\": the block with the text and \"Open\" to the tab", async () => {
    route({
      "/api/network/inspect": { label: "family", kind: "Peer", id_hex: "aa" },
      "/api/network/join": () => { throw new ApiError(409, "net_already_added", "x", { slot: "neta22", label: "family" }); },
    });
    mountForm();
    await typeInvite();
    await wrapper.get("#x-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#x-dup").text()).toContain('You\'re already in "family"');
    expect(wrapper.find("#x-error").attributes("hidden")).toBeDefined();
    await wrapper.get("#x-open").trigger("click");
    expect(location.hash).toBe("#peers/neta22");
    expect(done).toEqual([["here", null]]);
  });

  it("a 422 goes to the alert", async () => {
    route({
      "/api/network/inspect": () => { throw new ApiError(422, "invalid_key", "x"); },
      "/api/network/join": () => { throw new ApiError(422, "invalid_key", "x", { message: 'Bad "zebra"', word: "zebra" }); },
    });
    mountForm();
    await typeInvite();
    await wrapper.get("#x-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#x-error").attributes("role")).toBe("alert");
    expect(wrapper.get("#x-error mark").text()).toBe('"zebra"');
  });

  it("done carries the slot of the answer", async () => {
    route({ "/api/network/inspect": { label: "family", kind: "Peer", id_hex: "aa" }, "/api/network/join": { label: "family", restart: true, slot: "neta22" } });
    mountForm();
    await typeInvite();
    await wrapper.get("#x-btn").trigger("click");
    await flushPromises();
    expect(done).toEqual([["joined", "neta22"]]);
  });

  it("a double click sends one request", async () => {
    route({ "/api/network/inspect": { label: "family", kind: "Peer", id_hex: "aa" }, "/api/network/join": { label: "family", restart: true, slot: "neta22" } });
    mountForm();
    await typeInvite();
    await wrapper.get("#x-btn").trigger("click");
    await wrapper.get("#x-btn").trigger("click");
    await flushPromises();
    expect(posts().length).toBe(1);
  });
});
