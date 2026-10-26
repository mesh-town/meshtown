/* The "Leave network" row of Settings with a Managed network. A single network of an admin gets a red
 * "Delete network for everyone…" next to the leave button; with two or more networks the row is replaced by a pointer text
 * `To leave or delete a network, open Members → its tab.`; a plain member gets no delete button. After the
 * deletion the admin sees `You deleted "<net>". All members have left it.` with OK. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ANNA_FULL, NET_NAME, managedNode } from "../testing/managed-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";

const DELETE_LABEL = "Delete network for everyone…";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("one Managed network", () => {
  describe("an admin", () => {
    it("has the red delete button next to leave", async () => {
      const page = await open(managedNode({ meRole: "admin" }));
      expect(page.deleteButton().exists()).toBe(true);
      expect(page.deleteButton().classes()).toContain("btn-danger");
      expect(page.leaveButton().exists()).toBe(true);
    });

    it("click, name typed, confirm: sends the delete with confirm:true", async () => {
      const page = await open(managedNode({ meRole: "admin" }));
      await page.whenDeleteConfirmed();
      const sent = page.node.calls.filter((c) => c.method === "POST" && c.path.endsWith("/network/delete"));
      expect(sent).toHaveLength(1);
      expect(sent[0].body).toEqual({ confirm: true });
    });

    it("after the deletion says so and offers OK", async () => {
      const page = await open(managedNode({ meRole: "admin" }));
      await page.whenDeleteConfirmed();
      expect(page.rowText()).toContain('You deleted "work". All members have left it.');
      expect(page.wrapper.findAll("#sec-settings button").some((b) => b.text().trim() === "OK")).toBe(true);
    });
  });

  describe("a member", () => {
    it("has leave but no delete button", async () => {
      const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
      expect(page.leaveButton().exists()).toBe(true);
      expect(page.deleteButton().exists()).toBe(false);
    });
  });
});

async function open(node) {
  node.routes["POST /api/network/delete"] = {};
  const { wrapper } = await mountPage(node, { hash: "settings" });
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const byText = (text) => wrapper.findAll("#settings-leave-row button").find((b) => b.text().trim() === text);
  return {
    node, wrapper,
    rowText: () => wrapper.get("#sec-settings").text().replace(/\s+/g, " "),
    leaveButton: () => wrapper.find("#settings-leave-btn"),
    deleteButton: () => byText(DELETE_LABEL) ?? { exists: () => false },
    async whenDeleteConfirmed() {
      await click(byText(DELETE_LABEL));
      await wrapper.get("#member-reason").setValue(NET_NAME);
      await settle();
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
}
