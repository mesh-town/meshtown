/* `dialog-specs.ts`: the texts of the five confirmation dialogs (`dialogSpec(kind, ctx)`), exactly as in the UI spec, section 2.2.
 *
 * Contract fixed by these tests:
 *  - `dialogSpec(kind, ctx): { title: string, body: string[], hint?: string, action: string, danger: boolean }`
 *      `kind`: "remove" (3+ members, a proposal) | "remove-two" (a network of two, removed at once) | "confirm" (the second vote)
 *      | "block" | "leave".
 *      `ctx`: `{ name, by?, net?, isMyExit? }` — `name`: the target as shown in the list (name or "member <id>") — the `{n}` of the
 *      texts; `by`: who proposed (only "confirm"); `net`: the network name WITHOUT quotes (quotes are added by the spec;
 *      an absent/blank `net` drops the ' "..."' part, no "undefined" in any text); `isMyExit`: the target is my chosen exit.
 *  - `body` is a flat list of paragraphs, in this order. For "remove" the two bullet sentences are separate entries of `body`
 *      (the dialog may draw them as a list). The paragraph "{n} is your exit: ..." is the LAST entry of `body`, only when
 *      `isMyExit` is true and the kind is not "leave".
 *  - `hint` is a separate small paragraph under the body; absent (undefined) when the spec has none ("remove-two", "leave").
 *  - `action` is the label of the action button (the cancel button is always "Cancel" and is not part of the spec);
 *      `danger` is true for all five (the action button is red).
 *  - `spec.field?: { label: string, max: number }` — the optional text input the dialog draws before its buttons (the reason of a removal):
 *      present ONLY for "remove" (label "Reason (optional, members will see it)", max 80); absent (undefined) for "remove-two", "confirm",
 *      "block" and "leave".
 *  - `ctx.reason?: string | null` — the reason the proposer gave. Only "confirm" uses it: the line `Reason from {by}: "{reason}"` goes to
 *      `body` right after the main paragraph (before the "your exit" paragraph); no reason (absent, null, empty) — no line.
 *  - The "confirm" text has no time of the proposal after the network name. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { dialogSpec } from "./dialog-specs.ts";

const KINDS = ["remove", "remove-two", "confirm", "block", "leave"];
const ctx = (over = {}) => ({ name: "old-laptop", by: "peer-one", net: "family", isMyExit: false, ...over });
const EXIT_PARAGRAPH = "old-laptop is your exit: after this, sites won't open until you choose another exit.";

describe("remove (a proposal, 3+ members)", () => {
  it("asks to remove the member from the network", () => {
    assert.equal(spec("remove").title, "Remove old-laptop from the network?");
  });

  it("explains the two-member rule and what happens here and for everyone else", () => {
    assert.deepEqual(spec("remove").body, [
      'old-laptop will lose access to the network "family". This takes two members: you propose, and one more member confirms.',
      "On this computer, old-laptop is blocked right away.",
      "For everyone else, it's removed after a second member confirms. Its open connections are then cut.",
    ]);
  });

  it("advises \"Block\" for the case of keeping the member away from this computer only", () => {
    assert.equal(
      spec("remove").hint,
      'Use this if the computer is lost, stolen, or no longer used. To keep old-laptop away from this computer only, choose "Block on this computer" instead.',
    );
  });

  it("the buttons: \"Propose removal\" (red) and Cancel", () => {
    assert.equal(spec("remove").action, "Propose removal");
    assert.equal(spec("remove").danger, true);
  });
});

describe("remove-two (a network of two)", () => {
  it("the same title as a proposal", () => {
    assert.equal(spec("remove-two").title, "Remove old-laptop from the network?");
  });

  it("one paragraph: removed right away, can not be undone here", () => {
    assert.deepEqual(spec("remove-two").body, [
      "You and old-laptop are the only members, so old-laptop is removed right away and its open connections are cut. You can't undo this here.",
    ]);
  });

  it("no hint", () => {
    assert.ok(!spec("remove-two").hint, "hint: " + spec("remove-two").hint);
  });

  it("the button names the member: \"Remove old-laptop\" (red)", () => {
    assert.equal(spec("remove-two").action, "Remove old-laptop");
    assert.equal(spec("remove-two").danger, true);
  });
});

describe("confirm (the second vote)", () => {
  it("asks to confirm removing the member", () => {
    assert.equal(spec("confirm").title, "Confirm removing old-laptop?");
  });

  it("names the proposer, the network and the consequence of the second confirmation", () => {
    assert.deepEqual(spec("confirm").body, [
      'peer-one proposed removing old-laptop from the network "family". Yours is the second confirmation: old-laptop will be removed for everyone right away and its open connections will be cut.',
    ]);
  });

  it("advises to ask the proposer when unsure", () => {
    assert.equal(spec("confirm").hint, "Confirm only if you know why. If unsure, ask peer-one first.");
  });

  it("the button names the member: \"Remove old-laptop\" (red)", () => {
    assert.equal(spec("confirm").action, "Remove old-laptop");
    assert.equal(spec("confirm").danger, true);
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

describe("the reason field", () => {
  it("remove (a proposal) has a field with the label and the limit of 80", () => {
    assert.deepEqual(spec("remove").field, { label: "Reason (optional, members will see it)", max: 80 });
  });

  it.each(["remove-two", "confirm", "block", "leave"])("%s has no field", (kind) => {
    assert.equal(spec(kind).field, undefined, kind);
  });

  it("the field does not depend on the context", () => {
    assert.deepEqual(spec("remove", { isMyExit: true, net: "", reason: "lost laptop" }).field, spec("remove").field);
  });
});

describe("confirm (the second vote) with the reason of the proposer", () => {
  const LINE = 'Reason from peer-one: "lost laptop"';

  it("adds the reason line after the main paragraph", () => {
    const plain = spec("confirm").body;
    assert.deepEqual(spec("confirm", { reason: "lost laptop" }).body, [...plain, LINE]);
  });

  it("the line names the proposer given in the context", () => {
    const body = spec("confirm", { by: "router", reason: "lost laptop" }).body;
    assert.equal(body.at(-1), 'Reason from router: "lost laptop"');
  });

  it("the reason line comes before the \"your exit\" paragraph", () => {
    const plain = spec("confirm").body;
    assert.deepEqual(spec("confirm", { reason: "lost laptop", isMyExit: true }).body, [...plain, LINE, EXIT_PARAGRAPH]);
  });

  it("the reason is taken as it is: quotes and markup are not changed", () => {
    const body = spec("confirm", { reason: '<b>"old"</b>' }).body;
    assert.equal(body.at(-1), 'Reason from peer-one: "<b>"old"</b>"');
  });

  it.each([["absent", undefined], ["null", null], ["empty", ""]])("a reason that is %s adds no line", (_name, reason) => {
    assert.deepEqual(spec("confirm", { reason }).body, spec("confirm").body);
  });

  it("the title, the hint and the button are not changed", () => {
    const withReason = spec("confirm", { reason: "lost laptop" });
    assert.equal(withReason.title, spec("confirm").title);
    assert.equal(withReason.hint, spec("confirm").hint);
    assert.equal(withReason.action, spec("confirm").action);
  });

  it.each(["remove", "remove-two", "block", "leave"])("%s ignores a reason in the context", (kind) => {
    assert.deepEqual(spec(kind, { reason: "lost laptop" }), spec(kind));
  });
});

describe("the target is my chosen exit", () => {
  it.each(["remove", "remove-two", "confirm", "block"])("%s: a warning paragraph is added at the end of the body", (kind) => {
    const plain = spec(kind).body;
    const withExit = spec(kind, { isMyExit: true }).body;
    assert.deepEqual(withExit, [...plain, EXIT_PARAGRAPH]);
  });

  it("leave: no such paragraph (it is not about a member)", () => {
    assert.deepEqual(spec("leave", { isMyExit: true }), spec("leave"));
  });

  it("the rest of the spec is not changed", () => {
    const withoutBody = (s) => ({ ...s, body: null });
    for (const kind of ["remove", "confirm", "block"]) {
      assert.deepEqual(withoutBody(spec(kind, { isMyExit: true })), withoutBody(spec(kind)), kind);
    }
  });

  it("not my exit: the paragraph is absent", () => {
    for (const kind of KINDS) assert.ok(!spec(kind, { isMyExit: false }).body.includes(EXIT_PARAGRAPH), kind);
  });
});

describe("the names are taken from the context", () => {
  it("another member and another network", () => {
    const other = spec("remove", { name: "member cccc3333", net: "office" });
    assert.equal(other.title, "Remove member cccc3333 from the network?");
    assert.ok(other.body[0].startsWith('member cccc3333 will lose access to the network "office".'), other.body[0]);
    assert.ok(other.hint.includes("To keep member cccc3333 away from this computer only"), other.hint);
    assert.equal(other.action, "Propose removal");
  });

  it("another proposer in the confirmation", () => {
    const other = spec("confirm", { by: "router" });
    assert.ok(other.body[0].startsWith("router proposed removing old-laptop"), other.body[0]);
    assert.ok(other.hint.endsWith("ask router first."), other.hint);
  });
});

describe("no network name in the context", () => {
  it.each(["remove", "confirm", "leave"])("%s: no empty quotes, no \"undefined\"", (kind) => {
    for (const net of [undefined, "", "   "]) {
      const s = spec(kind, { net });
      for (const text of [s.title, ...s.body, s.hint || ""]) {
        assert.ok(!/undefined|null/.test(text), kind + " / " + String(net) + ": " + text);
        assert.ok(!/"\s*"/.test(text), kind + " / " + String(net) + ": empty quotes in " + text);
      }
    }
  });
});

describe("any kind", () => {
  it.each(KINDS)("%s: a title, a non-empty body of strings, an action label and a boolean danger", (kind) => {
    const s = spec(kind);
    assert.equal(typeof s.title, "string");
    assert.ok(Array.isArray(s.body) && s.body.length > 0 && s.body.every((p) => typeof p === "string" && p.length > 0));
    assert.equal(typeof s.action, "string");
    assert.equal(typeof s.danger, "boolean");
  });
});

// ---------------------------------------------------------------- setup

// The spec of `kind` for the default context (a member "old-laptop" proposed for removal by "peer-one" in the network "family").
function spec(kind, over = {}) {
  return dialogSpec(kind, ctx(over));
}
