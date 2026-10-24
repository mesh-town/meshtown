/* `dialog-specs.ts`: the dialogs of a managed network. Peer dialogs: `dialog-specs.test.mjs`.
 *
 * Contract fixed by these tests:
 *  - New `DialogKind`s: "make-admin", "give-up", "ban-managed" (remove a member), "delete-net", "last-admin".
 *  - `ctx.net` is the network name WITHOUT quotes; the spec adds them. `ctx.name` — the target as shown in the list.
 *  - "delete-net" also takes `ctx.members` (how many members the network has: the `{N}` of the text) and, when the target
 *      of the red button is my exit, `ctx.isMyExit` with `ctx.exitName` (the exit as shown in the list).
 *  - Only "delete-net" has an input (`field: { label, max }`): the name of the network typed to confirm.
 *  - "last-admin" has a second action button next to the main one: `spec.alt = { label, danger }`; the cancel button is never part
 *      of a spec. No other kind has `alt`.
 *  - The generic "{name} is your exit" paragraph is added to "ban-managed" like to the peer bans, and to no other new kind. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { dialogSpec } from "./dialog-specs.ts";

const NEW_KINDS = ["make-admin", "give-up", "ban-managed", "delete-net", "last-admin"];
const GENERIC_EXIT = "boris is your exit: after this, sites won't open until you choose another exit.";
const ctx = (over = {}) => ({ name: "boris", net: "work", isMyExit: false, members: 4, ...over });

describe("make-admin", () => {
  it("asks whether to make the member an admin of the network", () => {
    assert.equal(spec("make-admin").title, 'Make boris an admin of "work"?');
  });

  it("says what an admin can do", () => {
    assert.deepEqual(spec("make-admin").body, ["boris will be able to invite and remove members, make others admins and delete the network."]);
  });

  it("the main button is Make admin and it is not red", () => {
    assert.equal(spec("make-admin").action, "Make admin");
    assert.equal(spec("make-admin").danger, false);
  });

  it("has neither a field nor a hint", () => {
    assert.equal(spec("make-admin").field, undefined);
    assert.equal(spec("make-admin").hint, undefined);
  });

  // Open question: the generic "is your exit" line says sites stop opening, which is wrong for a role change; the test assumes it is not added.
  it("adds no paragraph about the exit even when the member is my exit", () => {
    assert.equal(spec("make-admin", { isMyExit: true }).body.length, 1);
  });
});

describe("give-up", () => {
  it("asks whether to give up the admin rights in the network", () => {
    assert.equal(spec("give-up").title, 'Give up admin rights in "work"?');
  });

  it("tells that the member stays and only another admin can bring the rights back", () => {
    assert.deepEqual(spec("give-up").body, ["You stay a member. Only another admin can make you an admin again."]);
  });

  it("the main button is Give up", () => {
    assert.equal(spec("give-up").action, "Give up");
  });

  it("has no field and no exit paragraph", () => {
    assert.equal(spec("give-up").field, undefined);
    assert.equal(spec("give-up", { isMyExit: true }).body.length, 1);
  });
});

describe("ban-managed", () => {
  it("asks whether to remove the member from the network", () => {
    assert.equal(spec("ban-managed").title, 'Remove boris from "work"?');
  });

  it("says the member is cut off at once and needs a new invite to return", () => {
    assert.deepEqual(spec("ban-managed").body, [
      "boris is disconnected from all members right away. To come back, it needs a new invite from an admin.",
    ]);
  });

  it("the main button is a red Remove", () => {
    assert.equal(spec("ban-managed").action, "Remove");
    assert.equal(spec("ban-managed").danger, true);
  });

  // Open question: for-D.md gives no reason input for the removal by an admin; the test assumes there is none.
  it("asks for no reason", () => {
    assert.equal(spec("ban-managed").field, undefined);
  });

  it("the member that is my exit adds the usual last paragraph", () => {
    const body = spec("ban-managed", { isMyExit: true }).body;
    assert.equal(body.at(-1), GENERIC_EXIT);
    assert.equal(body.length, 2);
  });
});

describe("delete-net", () => {
  it("asks whether to delete the network for everyone", () => {
    assert.equal(spec("delete-net").title, 'Delete "work" for everyone?');
  });

  it("counts the members who leave and says it cannot be undone", () => {
    assert.deepEqual(spec("delete-net").body, [
      'All 4 members leave "work", this computer too. This can\'t be undone: to be together again, someone has to create a new network and invite everyone.',
    ]);
  });

  it("takes the number of members from the context", () => {
    assert.ok(spec("delete-net", { members: 7 }).body[0].startsWith('All 7 members leave "work"'));
  });

  it("asks to type the network name", () => {
    const { field } = spec("delete-net");
    assert.equal(field.label, "Type the network name to confirm");
    assert.equal(typeof field.max, "number");
    assert.ok(field.max > 0, "the field takes no characters");
  });

  it("the main button is a red Delete network", () => {
    assert.equal(spec("delete-net").action, "Delete network");
    assert.equal(spec("delete-net").danger, true);
  });

  describe("my exit is in this network", () => {
    it("adds a last paragraph about sites opening directly", () => {
      // Open question: the context field of the exit's name is taken as `exitName`, the number of members as `members`.
      const body = spec("delete-net", { isMyExit: true, exitName: "anna" }).body;
      assert.equal(body.length, 2);
      assert.equal(
        body.at(-1),
        "Your exit anna is in this network. After the network is deleted, sites will open directly until you choose another exit.",
      );
    });

    it("does not add the generic paragraph of the bans", () => {
      const body = spec("delete-net", { isMyExit: true, exitName: "anna" }).body;
      assert.ok(!body.some((line) => line.includes("is your exit")), body.join(" | "));
    });
  });

  describe("my exit is elsewhere", () => {
    it("adds nothing", () => {
      assert.equal(spec("delete-net", { isMyExit: false, exitName: "anna" }).body.length, 1);
    });
  });
});

describe("last-admin", () => {
  it("tells this computer is the only admin of the network", () => {
    assert.equal(spec("last-admin").title, 'You\'re the only admin of "work"');
  });

  it("explains what leaving would cost and what to do first", () => {
    assert.deepEqual(spec("last-admin").body, [
      "If you leave, nobody can invite or remove members there. Make another member an admin first, or delete the network for everyone.",
    ]);
  });

  it("the main button is Choose an admin and it is not red", () => {
    assert.equal(spec("last-admin").action, "Choose an admin");
    assert.equal(spec("last-admin").danger, false);
  });

  it("the second button is a red Delete network…", () => {
    assert.deepEqual(spec("last-admin").alt, { label: "Delete network…", danger: true });
  });
});

describe("the other kinds", () => {
  it("have no second button", () => {
    for (const kind of ["make-admin", "give-up", "ban-managed", "delete-net", "ban", "block", "leave"]) {
      assert.equal(spec(kind).alt, undefined, kind + " has a second button");
    }
  });
});

describe("a network without a name", () => {
  it("leaves no 'undefined' in any text", () => {
    for (const kind of NEW_KINDS) {
      const text = JSON.stringify(dialogSpec(kind, { name: "boris", members: 4 }));
      assert.ok(!text.includes("undefined"), kind + ": " + text);
    }
  });
});

function spec(kind, over = {}) {
  return dialogSpec(kind, ctx(over));
}
