/* The invite card on Home (`#home-invite`) in a Managed network: a member who is not an admin gets the plaque
 * `Only admins can invite people to "<net>". Ask <a> or <b> to send an invite.` (`#peers-invite-note`) instead of the button
 * an admin can open the invite dialog. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ANNA_FULL, BOB_FULL, managedNode } from "../testing/managed-fixtures.mjs";
import { INVITE_NAME, openInviteDialog } from "../testing/invite-dialog.mjs";
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
    expect(card.root().find(INVITE_NAME).exists()).toBe(false);
    expect(card.root().text()).not.toContain("For whom");
  });
});

describe("an admin", () => {
  it("can open the name dialog", async () => {
    const card = await open(managedNode({ meRole: "admin" }));
    expect(card.button().exists()).toBe(true);
    const dialog = await openInviteDialog(card.root());
    expect(dialog.get(INVITE_NAME).element.value).not.toBe("");
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
