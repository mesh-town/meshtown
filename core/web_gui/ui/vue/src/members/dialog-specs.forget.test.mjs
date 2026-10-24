/* `dialog-specs.ts`: the removal dialogs (forget, forget-two, forget-confirm). The contract is in `dialog-specs.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { dialogSpec } from "./dialog-specs.ts";

const KINDS = ["ban", "ban-two", "ban-confirm", "block", "leave", "forget", "forget-two", "forget-confirm"];
const BAN_KINDS = ["ban", "ban-two", "ban-confirm"];
const FORGET_KINDS = ["forget", "forget-two", "forget-confirm"];
const ctx = (over = {}) => ({ name: "old-laptop", by: "peer-one", net: "family", isMyExit: false, ...over });
const EXIT_PARAGRAPH = "old-laptop is your exit: after this, sites won't open until you choose another exit.";

describe("forget (a proposal to remove a member from the list)", () => {
  const forget = (over = {}) => spec("forget", { seen: 777600, other: "peer-one", ...over });

  it("asks to remove the member from the network", () => {
    assert.equal(forget().title, "Remove old-laptop from the network?");
  });

  it("says when the member was last online and what removal does", () => {
    const text = forget().body.join(" ");
    assert.ok(text.includes("old-laptop was last online 9 d ago."), text);
    assert.ok(
      text.includes("It disappears from the member list on every computer in the network and can't connect until it joins again. To bring it back, send it a new invite."),
      text,
    );
  });

  it("explains that it takes two members and that the member is hidden here at once", () => {
    const text = forget().body.join(" ");
    assert.ok(
      text.includes("This takes two members: you propose, and one more member confirms. On this computer, old-laptop is hidden right away."),
      text,
    );
  });

  it("the button: \"Propose removal\" (red), no reason field", () => {
    assert.equal(forget().action, "Propose removal");
    assert.equal(forget().danger, true);
    assert.ok(!forget().field, "field: " + JSON.stringify(forget().field));
  });

  describe("a member who was online less than a week ago", () => {
    const HINT = "It was online recently \u2014 remove it only if this computer is gone.";

    it("gets a hint to remove it only if the computer is gone", () => {
      assert.equal(forget({ seen: 2 * 86400 }).hint, HINT);
    });

    it("also a minute ago", () => {
      assert.equal(forget({ seen: 60 }).hint, HINT);
    });

    it("a second below the week still gets it", () => {
      assert.equal(forget({ seen: 604799 }).hint, HINT);
    });

    it("exactly a week: no hint", () => {
      assert.ok(!forget({ seen: 604800 }).hint, "hint: " + forget({ seen: 604800 }).hint);
    });

    it("over a week: no hint", () => {
      assert.ok(!forget({ seen: 777600 }).hint, "hint: " + forget({ seen: 777600 }).hint);
    });
  });

  describe("a member who was never seen online", () => {
    it("the body says so instead of the time", () => {
      const text = forget({ seen: null }).body.join(" ");
      assert.ok(text.includes("old-laptop has never been seen online."), text);
      assert.ok(!/last online/.test(text), text);
    });

    it("no hint, no \"null\" or \"undefined\" anywhere", () => {
      const s = forget({ seen: null });
      assert.ok(!s.hint, "hint: " + s.hint);
      for (const text of [s.title, ...s.body, s.action]) assert.ok(!/undefined|null|NaN/.test(text), text);
    });
  });
});

describe("forget-two (a network of two)", () => {
  const two = (over = {}) => spec("forget-two", { seen: 777600, other: "peer-one", ...over });

  it("the same title as a proposal", () => {
    assert.equal(two().title, "Remove old-laptop from the network?");
  });

  it("names the other member and says the member is removed right away", () => {
    const text = two().body.join(" ");
    assert.ok(text.includes("You and peer-one are the only members, so old-laptop is removed right away."), text);
  });

  it("does not talk about a second member confirming", () => {
    assert.ok(!/two members|one more member/.test(two().body.join(" ")), two().body.join(" "));
  });

  it("still says when the member was last online and what removal does", () => {
    const text = two().body.join(" ");
    assert.ok(text.includes("old-laptop was last online 9 d ago."), text);
    assert.ok(text.includes("It disappears from the member list on every computer in the network"), text);
  });

  it("the hint for a recent member is the same", () => {
    assert.equal(two({ seen: 3600 }).hint, "It was online recently \u2014 remove it only if this computer is gone.");
    assert.ok(!two({ seen: 777600 }).hint);
  });

  it("the button names the member: \"Remove old-laptop\" (red)", () => {
    assert.equal(two().action, "Remove old-laptop");
    assert.equal(two().danger, true);
  });
});

describe("forget-confirm (the second vote)", () => {
  const confirm = (over = {}) => spec("forget-confirm", { seen: 777600, ...over });

  it("asks to confirm removing the member", () => {
    assert.equal(confirm().title, "Confirm removing old-laptop?");
  });

  it("says that the second vote removes the member for everyone right away", () => {
    assert.ok(confirm().body.join(" ").includes("Yours is the second vote: old-laptop is removed for everyone right away."), confirm().body.join(" "));
  });

  it("the button names the member: \"Remove old-laptop\" (red)", () => {
    assert.equal(confirm().action, "Remove old-laptop");
    assert.equal(confirm().danger, true);
  });

  it("no reason field", () => {
    assert.ok(!confirm().field, "field: " + JSON.stringify(confirm().field));
  });
});

describe("a ban waits for votes while the removal finishes a round", () => {
  const AT_RISK = "A ban is waiting for votes in this network. With old-laptop removed, fewer votes are needed, and it may pass right away.";

  it.each(["forget-two", "forget-confirm"])("%s with the flag: the warning is the last line of the body", (kind) => {
    const s = spec(kind, { seen: 777600, other: "peer-one", banAtRisk: true });
    assert.equal(s.body[s.body.length - 1], AT_RISK);
  });

  it.each(["forget-two", "forget-confirm"])("%s without the flag: no warning", (kind) => {
    for (const banAtRisk of [false, undefined]) {
      const s = spec(kind, { seen: 777600, other: "peer-one", banAtRisk });
      assert.ok(!s.body.join(" ").includes("A ban is waiting"), s.body.join(" "));
    }
  });

  it.each(["forget-two", "forget-confirm"])("%s: the flag changes only the body", (kind) => {
    const on = spec(kind, { seen: 777600, other: "peer-one", banAtRisk: true });
    const off = spec(kind, { seen: 777600, other: "peer-one", banAtRisk: false });
    assert.deepEqual({ ...on, body: null }, { ...off, body: null });
    assert.equal(on.body.length, off.body.length + 1);
  });
});

// The spec of `kind` for the default context.
function spec(kind, over = {}) {
  return dialogSpec(kind, ctx(over));
}
