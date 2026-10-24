/* The invite card on Home (`#home-invite`) in a Managed network: a member who is not an admin gets the plaque
 * `Only admins can invite people to "<net>". Ask <a> or <b> to send an invite.` (`#peers-invite-note`) instead of the button
 * and the "For whom" field; an admin and a Peer network keep both. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ANNA_FULL, BOB_FULL, managedNode, peerNode } from "../testing/managed-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("a member who is not an admin", () => {
  it("sees the plaque with the admins instead of the button", async () => {
    const card = await open(managedNode({ meRole: "member", admins: [ANNA_FULL, BOB_FULL] }));
    expect(card.note()).toBe('Only admins can invite people to "work". Ask anna or bob to send an invite.');
    expect(card.button().exists()).toBe(false);
  });

  it("one admin makes it Ask <a> to send an invite.", async () => {
    const card = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
    expect(card.note()).toBe('Only admins can invite people to "work". Ask anna to send an invite.');
  });

  it("has no For whom field", async () => {
    const card = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
    expect(card.root().find("#invite-for-input").exists()).toBe(false);
    expect(card.root().text()).not.toContain("For whom");
  });
});

describe("an admin", () => {
  it("keeps the button and the For whom field", async () => {
    const card = await open(managedNode({ meRole: "admin" }));
    expect(card.button().exists()).toBe(true);
    expect(card.root().find("#invite-for-input").exists()).toBe(true);
    expect(card.root().find("#peers-invite-note").exists()).toBe(false);
  });
});

describe("a Peer network", () => {
  it("keeps the button and the For whom field", async () => {
    const card = await open(peerNode());
    expect(card.button().exists()).toBe(true);
    expect(card.root().find("#invite-for-input").exists()).toBe(true);
    expect(card.root().find("#peers-invite-note").exists()).toBe(false);
  });
});

async function open(node) {
  const { wrapper } = await mountPage(node, { hash: "" });
  const root = () => wrapper.get("#home-invite");
  return {
    root,
    note: () => root().get("#peers-invite-note").text().replace(/\s+/g, " ").trim(),
    button: () => root().find("#netkey-invite-btn"),
  };
}
