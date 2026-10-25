/* The plate "this computer can't connect" also shows on the "Setup" tab of Home. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const them = { side: "them", reason: "no", at_unix: 1 };

async function openWizard(rows, refused = true) {
  const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ admission_refused: refused }), tab: "home" });
  await page.wrapper.get("#home-tab-wizard").trigger("click");
  await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
  return page;
}

describe("the plate on the Setup tab", () => {
  it("a refusal and nobody online: the plate is in Home", async () => {
    const page = await openWizard([selfRow(), peerOneRow({ online: false, last_refusal: them })]);
    expect(page.wrapper.get("#home-tabpanel").attributes("aria-labelledby")).toBe("home-tab-wizard");
    expect(page.wrapper.get("#home-root #admission-refused").text()).toContain("peer-one refuses to let it in");
  });

  it("the flag is off: no plate", async () => {
    const page = await openWizard([selfRow(), peerOneRow({ online: false, last_refusal: them })], false);
    expect(page.wrapper.find("#admission-refused").exists()).toBe(false);
  });
});
