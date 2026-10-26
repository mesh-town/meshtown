/* The plate "this computer can't connect" on the tab of a network: shown after the refusal has stood for 15 seconds. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "./peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const them = { side: "them", reason: "no", at_unix: 1 };
const gone = (over = {}) => ({ online: false, last_refusal: them, ...over });
const names = (list) => list.map((n) => peerOneRow(gone({ name: "peer-" + n, id: n.repeat(8), id_full: n.toUpperCase().repeat(64) })));
const wait = (ms) => vi.advanceTimersByTimeAsync(ms);

async function plate(rows, extra = {}, flag = true) {
  const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), ...rows], ...extra }), state: stateWith({ admission_refused: flag }) });
  await wait(ADMISSION_REFUSED_DELAY_MS);
  return page.wrapper.find("#sec-peers #admission-refused");
}

describe("the plate on the tab", () => {
  it("is not there before 15 seconds, then it is a status in the warning style", async () => {
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), ...names(["a"])] }), state: stateWith({ admission_refused: true }) });
    await wait(ADMISSION_REFUSED_DELAY_MS - 1);
    expect(page.wrapper.find("#sec-peers #admission-refused").exists()).toBe(false);
    await wait(1);
    const el = page.wrapper.get("#sec-peers #admission-refused");
    expect(el.attributes("role")).toBe("status");
    expect(el.classes()).toContain("warn-card");
  });

  it("one name: who refuses and whom to ask", async () => {
    const el = await plate(names(["a"]));
    const text = el.text();
    expect(text).toContain("This computer can't connect to");
    expect(text).toContain("peer-a refuses to let it in, and no other member of");
    expect(text).toContain("Ask an admin of");
    expect(text).toContain("Until then, sites won't open through");
  });

  it("several names: the list", async () => {
    const el = await plate(names(["a", "b"]));
    expect(el.text()).toContain("peer-a and peer-b refuse to let it in");
  });

  it("more than three names: three and `and N more`", async () => {
    const el = await plate(names(["a", "b", "c", "d", "e"]));
    expect(el.text()).toContain("peer-a, peer-b, peer-c and 2 more refuse to let it in");
  });

  it("the reason \"unlisted\" gives no name: the plate says that members refuse", async () => {
    const el = await plate([peerOneRow(gone({ last_refusal: { side: "them", reason: "unlisted", at_unix: 1 } }))]);
    expect(el.text()).toContain("refuse to let it in, and no other member is online.");
    expect(el.text()).toContain("Ask an admin of");
  });

  it("the flag is off, though the list shows a refusal: no plate", async () => {
    const el = await plate([peerOneRow(gone()), peerTwoRow({ online: true })], {}, false);
    expect(el.exists()).toBe(false);
  });
});
