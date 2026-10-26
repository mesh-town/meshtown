/* The red strip of notices (`#notice-banner`, role alert) above Members and on Home: one line per deleted network,
 * `"<net>" was deleted by an admin (<by_name>). This computer has left it.` and an `OK` button (`#notice-ok`) that sends
 * `POST /api/notices/ack {id}` and, once the node stops listing the notice, makes the line go away. Oldest first.
 * The notices ride on the overview (`notices[]`) into `store.notices`. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { deletedNotice, managedNode } from "../testing/managed-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";

const TEXT = '"family" was deleted by an admin (anna). This computer has left it.';

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("a notice about a deleted network", () => {
  describe("on the Members tab", () => {
    it("shows the strip with the admin's name and OK", async () => {
      const page = await open(managedNode({ notices: [deletedNotice()] }), "peers");
      expect(page.strip().attributes("role")).toBe("alert");
      expect(page.strip().text()).toContain(TEXT);
      expect(page.ok().text()).toBe("OK");
    });

    it("an empty admin name leaves the name out", async () => {
      const page = await open(managedNode({ notices: [deletedNotice({ by_name: "" })] }), "peers");
      expect(page.strip().text()).toContain('"family" was deleted by an admin. This computer has left it.');
    });

    it("a null admin name leaves the name out too", async () => {
      const page = await open(managedNode({ notices: [deletedNotice({ by_name: null })] }), "peers");
      expect(page.strip().text()).not.toContain("(");
    });
  });

  describe("on Home", () => {
    it("shows the same strip", async () => {
      const page = await open(managedNode({ notices: [deletedNotice()] }), "");
      expect(page.strip().text()).toContain(TEXT);
    });
  });

  describe("OK", () => {
    it("acknowledges the notice by id", async () => {
      const page = await open(managedNode({ notices: [deletedNotice({ id: 42 })] }), "peers");
      await page.whenOkClicked();
      expect(page.acks()).toEqual([{ id: 42 }]);
    });

    it("makes the strip disappear", async () => {
      const page = await open(managedNode({ notices: [deletedNotice()] }), "peers");
      await page.whenOkClicked();
      expect(page.wrapper.find("#notice-banner").exists()).toBe(false);
    });

    it("stays away after the next poll", async () => {
      const page = await open(managedNode({ notices: [deletedNotice()] }), "peers");
      await page.whenOkClicked();
      await polls(page.node, 1);
      expect(page.wrapper.find("#notice-banner").exists()).toBe(false);
    });
  });

  describe("several notices", () => {
    it("are listed from the oldest to the newest", async () => {
      const older = deletedNotice({ id: 1, label: "first", at_unix: 1792400000 });
      const newer = deletedNotice({ id: 2, label: "second", at_unix: 1792500000 });
      const page = await open(managedNode({ notices: [newer, older] }), "peers");
      const text = page.strip().text();
      expect(text.indexOf('"first"'), "the older one comes first").toBeGreaterThanOrEqual(0);
      expect(text.indexOf('"first"')).toBeLessThan(text.indexOf('"second"'));
    });

    it("each has its own OK", async () => {
      const page = await open(managedNode({ notices: [deletedNotice({ id: 1 }), deletedNotice({ id: 2, label: "second" })] }), "peers");
      expect(page.wrapper.findAll("#notice-ok")).toHaveLength(2);
    });
  });

  describe("no notices", () => {
    it("no strip", async () => {
      const page = await open(managedNode(), "peers");
      expect(page.wrapper.find("#notice-banner").exists()).toBe(false);
    });
  });
});

async function open(node, hash) {
  const { wrapper, store } = await mountPage(node, { hash });
  const click = async (w) => { await w.trigger("click"); await settle(); };
  return {
    node, wrapper, store,
    strip: () => wrapper.get("#notice-banner"),
    ok: () => wrapper.get("#notice-ok"),
    acks: () => node.calls.filter((c) => c.method === "POST" && c.path === "/api/notices/ack").map((c) => c.body),
    whenOkClicked: () => click(wrapper.get("#notice-ok")),
  };
}
