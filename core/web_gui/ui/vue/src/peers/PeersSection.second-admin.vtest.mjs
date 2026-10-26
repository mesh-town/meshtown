/* The "name a second admin" hint on the Members tab, the whole page on a fake node: it shows while this computer is the only
 * admin, goes away once a second admin exists, and stays when "Make admin" is refused. Component: `SecondAdminHint.vtest.mjs`. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ANNA_FULL, ME_FULL, managedMembers, managedNode, managedState } from "../testing/managed-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

const HINT = "#second-admin-hint";

describe("the second-admin hint on the page", () => {
  it("disappears after make-admin succeeds", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL] }));
    expect(page.has(HINT)).toBe(true);
    page.node.routes["POST /api/members/admin"] = () => {
      page.node.state = managedState({ meRole: "admin", admins: [ME_FULL, ANNA_FULL] });
      page.node.routes["GET /api/members"] = managedMembers(page.node.state);
      return { ...page.node.routes["GET /api/members"], changed: true };
    };
    await page.makeAdmin("anna");
    await polls(page.node, 1);
    expect(page.has(HINT)).toBe(false);
  });

  it("stays when make-admin is refused", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL] }));
    page.node.routes["POST /api/members/admin"] = { status: 409, body: { error: "not_admin", message: "Only admins can do this." } };
    await page.makeAdmin("anna");
    expect(page.has(HINT)).toBe(true);
    expect(page.wrapper.get("#peers-exit-status").text()).toBe("Only admins can do this.");
  });

  it("is hidden, and the admin menu is gone, after my role is revoked", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL] }));
    expect(page.has(HINT)).toBe(true);
    page.node.state = managedState({ meRole: "member", admins: [ANNA_FULL] });
    page.node.routes["GET /api/members"] = managedMembers(page.node.state);
    await polls(page.node, 1);
    expect(page.has(HINT)).toBe(false);
    await page.openMenuOf("anna");
    expect(page.menuKeys()).not.toContain("make-admin");
  });

  it("is not shown to a member", async () => {
    const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
    expect(page.has(HINT)).toBe(false);
  });
});

async function open(node) {
  const { wrapper } = await mountPage(node, { hash: "peers" });
  const toggleOf = (name) => wrapper.findAll("#peers-list > li").find((r) => r.text().includes(name))?.get("button[data-member-menu]");
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const page = {
    node, wrapper,
    has: (sel) => wrapper.find("#sec-peers " + sel).exists(),
    menuKeys: () => wrapper.findAll('[role="menu"] [data-member-item]').map((i) => i.attributes("data-member-item")),
    async openMenuOf(name) {
      await click(toggleOf(name));
    },
    async makeAdmin(name) {
      await page.openMenuOf(name);
      await click(wrapper.get('[role="menu"] [data-member-item="make-admin"]'));
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
  return page;
}
