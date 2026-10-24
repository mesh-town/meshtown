/* A computer removed from the member list (`me_removed`): Home shows the card and the invite form, and the button of the card leads to the field.
 * Selectors and helpers: `PeersSection.members-leave.vtest.mjs`, `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { threeMembers, openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const removedMe = () => threeMembers({ me_removed: true });

describe("this computer was removed from the member list", () => {
  it("Home shows the card and the invite form with its own title and hint", async () => {
    const page = await openMembersPage({ members: removedMe(), tab: "home" });
    expect(page.has("#excluded-card")).toBe(true);
    expect(page.has("#join-key-input")).toBe(true);
    expect(page.text("#join-card-title")).toMatch(/^Join ".+" again$/);
    expect(page.text("#net-restart-hint")).not.toMatch(/restart|disconnect|exclu|ban/i);
  });

  it("\"Join with an invite\" opens Home and focuses the invite field", async () => {
    const page = await openMembersPage({ members: removedMe() });
    await page.click("#excluded-join");
    expect(page.tabShown("home")).toBe(true);
    expect(page.focusedId()).toBe("join-key-input");
  });

  it("the Home card and the Members card never repeat an id", async () => {
    const page = await openMembersPage({ members: removedMe(), tab: "home" });
    const ids = [...page.wrapper.element.querySelectorAll("[id]")].map((el) => el.id);
    expect(ids.filter((id, i) => ids.indexOf(id) !== i)).toEqual([]);
    expect(page.wrapper.element.querySelectorAll("#excluded-card, #members-removed-card").length).toBe(2);
  });

  it("the form button of the removed card says \"Join\"", async () => {
    const page = await openMembersPage({ members: removedMe(), tab: "home" });
    expect(page.text("#join-btn")).toBe("Join");
  });

  it("a banned computer in a Peer network is sent to another network", async () => {
    const page = await openMembersPage({ members: threeMembers({ me_excluded: true }), state: stateWith({ me_excluded: true }), tab: "home" });
    expect(page.text("#join-card-title")).toBe("Join another network");
  });
});
