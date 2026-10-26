/* `dialog-specs.ts`: the texts of the confirmation dialogs (`dialogSpec(kind, ctx)`) that are not about roles (those are in
 * `dialog-specs.managed.test.mjs`).
 *
 *  - `dialogSpec(kind, ctx): { title, body: string[], hint?, action, danger }`; `kind`: "admit" | "block" | "leave".
 *    `ctx`: `{ name, net?, isMyExit?, label?, multi?, exitName? }` — `name`: the target as shown in the list; `net`: the network
 *    name WITHOUT quotes (an absent or blank `net` drops the ' "..."' part, no "undefined" in any text).
 *  - The paragraph "{n} is your exit: ..." is the LAST entry of `body`, only when `isMyExit` is true and the kind is not "leave".
 *  - `danger` is true for all of them but "admit". No dialog here has a field. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { dialogSpec } from "./dialog-specs.ts";

const KINDS = ["admit", "block", "leave"];
const ctx = (over = {}) => ({ name: "old-laptop", net: "family", isMyExit: false, ...over });
const EXIT_PARAGRAPH = "old-laptop is your exit: after this, sites won't open until you choose another exit.";

describe("admit", () => {
  it("asks to confirm the member joining, and the join is immediate", () => {
    const s = spec("admit");
    assert.equal(s.title, "Confirm old-laptop joining?");
    assert.equal(s.body[0], 'old-laptop asked to join the network "family". After your confirmation it joins right away.');
    assert.equal(s.action, "Confirm old-laptop");
    assert.equal(s.danger, false);
  });

  it("adds the invite label line when there is one", () => {
    assert.deepEqual(spec("admit", { label: "Anna's phone" }).body.slice(1), ['Invited as "Anna\'s phone"']);
  });
});

describe("block", () => {
  it("asks to block the member on this computer", () => {
    assert.equal(spec("block").title, "Block old-laptop on this computer?");
  });

  it("says what the member can no longer do", () => {
    assert.deepEqual(spec("block").body, [
      "old-laptop won't be able to connect to this computer or open sites through it. Its open connections to you are cut.",
    ]);
  });

  it("says that only this computer is affected and points to \"Remove from network\"", () => {
    assert.equal(
      spec("block").hint,
      'Only this computer is affected: old-laptop stays in the network and keeps working with other members. To remove it for everyone, choose "Remove from network" instead.',
    );
  });

  it("the button names the member: \"Block old-laptop\" (red)", () => {
    assert.equal(spec("block").action, "Block old-laptop");
    assert.equal(spec("block").danger, true);
  });
});

describe("leave", () => {
  it("asks to leave the network, by its name", () => {
    assert.equal(spec("leave").title, 'Leave the network "family"?');
  });

  it("says that all members disconnect and a new invite is needed to come back", () => {
    assert.deepEqual(spec("leave").body, [
      "This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
    ]);
  });

  it("no hint", () => {
    assert.ok(!spec("leave").hint, "hint: " + spec("leave").hint);
  });

  it("the button: \"Leave network\" (red)", () => {
    assert.equal(spec("leave").action, "Leave network");
    assert.equal(spec("leave").danger, true);
  });
});

describe("leave in a page with several networks", () => {
  const MULTI = { multi: true, net: "my devices" };
  const OTHERS_STAY = "Disconnect this computer from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.";

  it("the title is short and names the network", () => {
    assert.equal(spec("leave", MULTI).title, 'Leave "my devices"?');
  });

  it("the body says that the other networks stay", () => {
    assert.deepEqual(spec("leave", MULTI).body, [OTHERS_STAY]);
  });

  it("the button is still red \"Leave network\", no hint and no field", () => {
    const s = spec("leave", MULTI);
    assert.equal(s.action, "Leave network");
    assert.equal(s.danger, true);
    assert.ok(!s.hint, "hint: " + s.hint);
    assert.ok(!s.field, "field: " + JSON.stringify(s.field));
  });

  describe("the exit is a member of this network", () => {
    it("a second paragraph names the exit", () => {
      assert.deepEqual(spec("leave", { ...MULTI, exitName: "peer-one" }).body, [
        OTHERS_STAY,
        "Your exit peer-one is in this network. After you leave, sites will open directly until you choose another exit.",
      ]);
    });

    it("the rest of the spec does not change", () => {
      const withoutBody = (s) => ({ ...s, body: null });
      assert.deepEqual(withoutBody(spec("leave", { ...MULTI, exitName: "peer-one" })), withoutBody(spec("leave", MULTI)));
    });
  });

  describe("the exit is in another network", () => {
    it.each([undefined, null, ""])("exitName %j: no paragraph about the exit", (exitName) => {
      assert.deepEqual(spec("leave", { ...MULTI, exitName }).body, [OTHERS_STAY]);
    });
  });

  it("`isMyExit` adds nothing (the exit has its own paragraph)", () => {
    assert.deepEqual(spec("leave", { ...MULTI, isMyExit: true }), spec("leave", MULTI));
  });
});

describe("leave in a page with one network is the same as before", () => {
  const PLAIN = spec("leave");

  it.each([{ multi: false }, { multi: undefined }, { exitName: null }])("context %j gives the plain spec", (over) => {
    assert.deepEqual(spec("leave", over), PLAIN);
  });

  it("an exit name without `multi` is ignored", () => {
    assert.deepEqual(spec("leave", { exitName: "peer-one" }), PLAIN);
    assert.deepEqual(spec("leave", { multi: false, exitName: "peer-one" }), PLAIN);
  });

  it("the sentences are unchanged, word for word", () => {
    assert.equal(PLAIN.title, 'Leave the network "family"?');
    assert.deepEqual(PLAIN.body, [
      "This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
    ]);
  });

  it.each(["admit", "block"])("%s ignores `multi`", (kind) => {
    assert.deepEqual(spec(kind, { multi: true, exitName: "peer-one" }), spec(kind));
  });
});
describe("the target is my chosen exit", () => {
  it.each(["admit", "block"])("%s: a warning paragraph is added at the end of the body", (kind) => {
    const plain = spec(kind).body;
    const withExit = spec(kind, { isMyExit: true }).body;
    assert.deepEqual(withExit, [...plain, EXIT_PARAGRAPH]);
  });

  it("leave: no such paragraph (it is not about a member)", () => {
    assert.deepEqual(spec("leave", { isMyExit: true }), spec("leave"));
  });

  it("not my exit: the paragraph is absent", () => {
    for (const kind of KINDS) assert.ok(!spec(kind, { isMyExit: false }).body.includes(EXIT_PARAGRAPH), kind);
  });
});

describe("the names are taken from the context", () => {
  it("another member and another network", () => {
    const other = spec("block", { name: "member cccc3333", net: "office" });
    assert.equal(other.title, "Block member cccc3333 on this computer?");
    assert.equal(other.action, "Block member cccc3333");
  });
});

describe("no network name in the context", () => {
  it.each(["admit", "leave"])("%s: no empty quotes, no \"undefined\"", (kind) => {
    for (const net of [undefined, "", "   "]) {
      const s = spec(kind, { net });
      for (const text of [s.title, ...s.body, s.hint || ""]) {
        assert.ok(!/undefined|null/.test(text), kind + " / " + String(net) + ": " + text);
        assert.ok(!/"\s*"/.test(text), kind + " / " + String(net) + ": empty quotes in " + text);
      }
    }
  });
});

describe("no vote wording", () => {
  it.each(KINDS)("%s: no vote, proposal or second-member wording", (kind) => {
    const s = spec(kind);
    for (const text of [s.title, ...s.body, s.hint || "", s.action]) assert.ok(!/vot|propos|second member/i.test(text), kind + ": " + text);
  });
});

describe("any kind", () => {
  it.each(KINDS)("%s: a title, a non-empty body of strings, an action label and a boolean danger, no field", (kind) => {
    const s = spec(kind);
    assert.equal(typeof s.title, "string");
    assert.ok(Array.isArray(s.body) && s.body.length > 0 && s.body.every((p) => typeof p === "string" && p.length > 0));
    assert.equal(typeof s.action, "string");
    assert.equal(typeof s.danger, "boolean");
    assert.equal(s.field, undefined);
  });
});

// The spec of `kind` for the default context (a member "old-laptop" in the network "family").
function spec(kind, over = {}) {
  return dialogSpec(kind, ctx(over));
}
