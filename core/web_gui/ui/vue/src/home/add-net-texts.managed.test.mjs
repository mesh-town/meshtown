/* The banner after joining a network: `You joined "<net>". Only admins invite and remove members here. Admins: <a>, <b>.`
 * `joinedBanner(net, via, admins)` takes the admins' display names as the third argument; without it the banner is as before.
 * The exit tail (`Your exit hasn't changed: ...`) stays where it was and is not duplicated. */

import { describe, expect, it } from "vitest";
import { joinedBanner } from "./add-net-texts.ts";

const MANAGED = 'You joined "work". Only admins invite and remove members here. Admins: anna, bob.';
const VIA = { peer: "peer-one", net: "friends" };
const TAIL = "Your exit hasn't changed: sites still open via peer-one (friends).";

describe("joinedBanner", () => {
  describe("without admins", () => {
    it("is the plain sentence", () => {
      expect(joinedBanner("work", null)).toBe('You joined "work".');
    });

    it("an empty list of admins is the same", () => {
      expect(joinedBanner("work", null, [])).toBe('You joined "work".');
    });
  });

  describe("with admins", () => {
    it("says who invites and lists the admins", () => {
      expect(joinedBanner("work", null, ["anna", "bob"])).toBe(MANAGED);
    });

    it("one admin is listed alone", () => {
      expect(joinedBanner("work", null, ["anna"])).toBe('You joined "work". Only admins invite and remove members here. Admins: anna.');
    });

    it("keeps the exit tail once", () => {
      const text = joinedBanner("work", VIA, ["anna", "bob"]);
      expect(text).toContain(MANAGED.replace(" Admins: anna, bob.", ""));
      expect(text).toContain("Admins: anna, bob.");
      expect(text.split(TAIL)).toHaveLength(2);
    });
  });

  it("without admins an exit keeps the old text exactly", () => {
    expect(joinedBanner("work", VIA)).toBe(`You joined "work". ${TAIL}`);
  });
});
