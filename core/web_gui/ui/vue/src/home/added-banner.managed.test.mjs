/* The banner after joining a network in the live path: `addedBannerText` and `arrivedBannerText` take the names
 * of the admins from the slot's snapshot and member list; a slot without admins gets the plain sentence. */

import { describe, expect, it } from "vitest";
import { MAIN, setup } from "../core/overview-test-support.mjs";
import { memberRow, membersReply } from "../testing/members-fixtures.mjs";
import { addedBannerText, arrivedBannerText } from "./add-net-actions.ts";

const ANNA = "b1b1b1b1" + "0".repeat(56);
const BOB = "c2c2c2c2" + "0".repeat(56);
const GHOST = "d3d3d3d3" + "0".repeat(56);
const ROWS = [
  memberRow({ id: "b1b1b1b1", id_full: ANNA, name: "anna" }),
  memberRow({ id: "c2c2c2c2", id_full: BOB, name: "bob" }),
];

function open(network) {
  return setup([{ id: MAIN, state: { network: { id_hex: "3f2a01", ...network } }, members: membersReply({ rows: ROWS }) }]).store;
}

describe("arrivedBannerText", () => {
  it("lists the admins by name", () => {
    const store = open({ admins: [ANNA, BOB] });
    expect(arrivedBannerText(store, MAIN)).toBe('You joined "friends". Only admins invite and remove members here. Admins: anna, bob.');
  });

  it("an admin the list does not know goes by the short id", () => {
    const store = open({ admins: [ANNA, GHOST] });
    expect(arrivedBannerText(store, MAIN)).toContain("Admins: anna, d3d3d3d3.");
  });

  it("a network with no known admins gets the plain sentence", () => {
    expect(arrivedBannerText(open({}), MAIN)).toBe('You joined "friends".');
  });
});

describe("addedBannerText", () => {
  it("a finished join names the admins", () => {
    const store = open({ admins: [ANNA] });
    const form = { slot: MAIN, netLabel: "friends" };
    expect(addedBannerText(store, form, "joined")).toBe('You joined "friends". Only admins invite and remove members here. Admins: anna.');
  });
});
