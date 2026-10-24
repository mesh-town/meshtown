/* `adminsLineText(admins, me, names)` — the text of `#admins-line` on the Members tab of a Managed network:
 * `Managed network · Admins: you, anna`. `admins` are full ids, `me` is this computer's full id, `names` maps a full id
 * to the name the page knows; an id without a name shows as its short form (its first characters). */

import { describe, expect, it } from "vitest";
import { adminsLineText } from "./admins-line.ts";

const ME = "a".repeat(64);
const ANNA = "b1b1b1b1" + "0".repeat(56);
const NONAME = "d3d3d3d3" + "0".repeat(56);

describe("adminsLineText", () => {
  describe("this computer is an admin", () => {
    it("is called you, the others go by name", () => {
      expect(adminsLineText([ME, ANNA], ME, { [ANNA]: "anna" })).toBe("Managed network · Admins: you, anna");
    });

    it("the only admin is just you", () => {
      expect(adminsLineText([ME], ME, {})).toBe("Managed network · Admins: you");
    });
  });

  describe("this computer is not an admin", () => {
    it("lists the admins by name", () => {
      expect(adminsLineText([ANNA], ME, { [ANNA]: "anna" })).toBe("Managed network · Admins: anna");
    });

    it("never says you", () => {
      expect(adminsLineText([ANNA], ME, { [ANNA]: "anna" })).not.toContain("you");
    });
  });

  describe("an admin without a known name", () => {
    it("shows the short id", () => {
      expect(adminsLineText([ANNA, NONAME], ME, { [ANNA]: "anna" })).toContain("d3d3d3d3");
    });

    it("an empty name counts as unknown", () => {
      expect(adminsLineText([NONAME], ME, { [NONAME]: "" })).toContain("d3d3d3d3");
    });
  });

  it("keeps the order of the list it was given", () => {
    expect(adminsLineText([ANNA, ME], ME, { [ANNA]: "anna" })).toBe("Managed network · Admins: anna, you");
  });
});
