/* An invite in the page address (`?invite=`): read once, kept for the forms, sent to "Add another network" when the node is in a network. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createStore } from "../core/store.ts";
import { STATE } from "../shell/use-shell-support.mjs";

const INVITE = "mesh-invite2-abc";
const IN_NET = { ...STATE, network: { id_hex: "net-aaaa", masked: "mesh1-••••-••••" } };

// The address is read once per module: every test gets a fresh copy of the modules.
async function fresh() {
  vi.resetModules();
  const invite = await import("./use-invite-join.ts");
  const shell = await import("../shell/use-shell-support.mjs");
  return { invite, shell };
}

beforeEach(() => {
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
