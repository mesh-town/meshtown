/* The words of the invite form, the Home cards and the Members page never name the mechanism of the exclusion:
 * no "node's key", "key is excluded" or "excluded" in what a person reads (the word may stay in constant names). */

import assert from "node:assert/strict";
import { describe, it } from "vitest";
import * as card from "./home-card-texts.ts";
import * as invite from "./invite-join-texts.ts";
import * as members from "../members/members-texts.ts";

const BAD = [/node'?s key/i, /key is excluded/i, /excluded/i];

// Every exported string, and the result of every exported function called with a network name (and numbers where it takes them).
function words(mod) {
  const out = [];
  for (const [name, value] of Object.entries(mod)) {
    if (typeof value === "string") out.push([name, value]);
    if (typeof value !== "function") continue;
    for (const args of [["family", "family"], [3, 3], [["a", "b"], "family"]]) {
      try {
        const text = value(...args);
        if (typeof text === "string") out.push([name, text]);
      } catch {
        // the arguments do not fit this function: another set is tried
      }
    }
  }
  return out;
}

describe("the words people read", () => {
  for (const [label, mod] of [["invite-join-texts", invite], ["home-card-texts", card], ["members-texts", members]]) {
    it(`${label}: no "node's key" / "key is excluded" / "excluded"`, () => {
      const list = words(mod);
      assert.ok(list.length > 5, "the texts were collected");
      for (const [name, text] of list) for (const re of BAD) assert.ok(!re.test(text), `${name}: ${text}`);
    });
  }

  it("the refusal lead names the network or none", () => {
    assert.equal(invite.REFUSED_LEAD(' "m"'), 'Couldn\'t join "m".');
    assert.equal(invite.REFUSED_LEAD(""), "Couldn't join.");
  });

  it("the lines of the table", () => {
    assert.equal(card.HOME_NO_RETURN_HINT("n"), 'This computer can\'t rejoin "n". To keep using Mesh Town, paste an invite to another network.');
    assert.equal(invite.NOTE_NEW_AT_LIMIT, "This is a new network, and Mesh Town is already in 8, the most it can be in. Leave a network first.");
    assert.equal(invite.ADD_NET_LIMIT_HERE, "This is a new network, so it can't be added until you leave one.");
  });
});
