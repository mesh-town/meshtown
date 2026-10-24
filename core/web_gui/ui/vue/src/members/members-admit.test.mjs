/* Join requests ("admission") in the members logic: parsing, errors, the confirm rules, grouping, row decoration and the
 * menu. Contract: `GET /api/members` rows may have status "pending" and an `admission` object; a reply may carry
 * `me_pending` / `me_admission` and `counts.pending`; an old reply without them still parses. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { memberErrorText, parseMembers, postMemberAction } from "./members-api.ts";
import { admittable, canAdmit, canUnadmit, menuItems, partitionRows, rowDecor } from "./members-view.ts";
import {
  EXTRA_FULL,
  admissionInfo,
  extraRow,
  foreignVote,
  membersReply,
  peerOneRow,
  pendingRow,
  selfRow,
} from "../testing/members-fixtures.mjs";
import { PEER_FULL, SELF_FULL } from "../testing/section.mjs";

const PENDING_FULL = "E".repeat(64);

describe("parseMembers: join requests", () => {
  it("parses pending rows and admission", () => {
    const label = "Anna's laptop";
    const reply = membersReply({
      rows: [selfRow(), pendingRow({ admission: admissionInfo({ voters: [PEER_FULL], label }) })],
      me_pending: true,
      me_admission: admissionInfo({ votes: 0, voters: [] }),
    });
    const parsed = parseMembers(reply);
    assert.deepEqual(parsed, reply);
    assert.equal(parsed.members[1].status, "pending");
    assert.equal(parsed.counts.pending, 1);
  });

  it("an old reply without the new fields parses", () => {
    const old = membersReply();
    for (const k of ["me_pending", "me_admission"]) delete old[k];
    delete old.counts.pending;
    for (const r of old.members) delete r.admission;
    const parsed = parseMembers(old);
    assert.equal(parsed.me_pending, false);
    assert.equal(parsed.me_admission, null);
    assert.equal(parsed.counts.pending, 0);
    assert.equal(parsed.members[0].admission, null);
  });

  it("counts.pending falls back to the number of pending rows", () => {
    const raw = membersReply({ rows: [selfRow(), pendingRow()] });
    delete raw.counts.pending;
    assert.equal(parseMembers(raw).counts.pending, 1);
  });

  it("a blank label is null, a broken admission is null", () => {
    const raw = membersReply({ rows: [pendingRow({ admission: { votes: 1, need: 2, voters: [], mine: false, label: "  " } }), extraRow({ admission: "x" })] });
    const parsed = parseMembers(raw);
    assert.equal(parsed.members[0].admission.label, null);
    assert.equal(parsed.members[1].admission, null);
  });
});

describe("admit and unadmit", () => {
  it("post to the action path with the id", async () => {
    const calls = [];
    const api = async (method, path, body) => (calls.push({ method, path, body }), { ...membersReply(), changed: true });
    const out = await postMemberAction(api, "admit", PENDING_FULL);
    assert.deepEqual(calls[0], { method: "POST", path: "/api/members/admit", body: { id: PENDING_FULL } });
    assert.equal(out.changed, true);
    await postMemberAction(api, "unadmit", PENDING_FULL);
    assert.equal(calls[1].path, "/api/members/unadmit");
    assert.deepEqual(calls[1].body, { id: PENDING_FULL });
  });

  it("admit/unadmit errors have human reasons", () => {
    const err = (code) => new ApiError(409, code, code);
    assert.equal(memberErrorText(err("not_pending")), "Couldn't update the member list: this node isn't waiting for confirmation");
    assert.equal(
      memberErrorText(err("not_confirmed")),
      "Couldn't update the member list: you haven't confirmed this node, or it has already joined",
    );
  });
});

describe("canAdmit / canUnadmit / admittable", () => {
  it("canAdmit: pending and not confirmed by me", () => {
    assert.equal(canAdmit(pendingRow(), SELF_FULL), true);
  });

  it("canAdmit is false for my own confirmation, a member, an excluded row, myself and null", () => {
    assert.equal(canAdmit(pendingRow({ admission: admissionInfo({ mine: true }) }), SELF_FULL), false);
    assert.equal(canAdmit(extraRow({ admission: admissionInfo() }), SELF_FULL), false);
    assert.equal(canAdmit(extraRow({ status: "excluded" }), SELF_FULL), false);
    assert.equal(canAdmit(pendingRow({ id_full: SELF_FULL }), SELF_FULL), false);
    assert.equal(canAdmit(pendingRow({ admission: null }), SELF_FULL), false);
    assert.equal(canAdmit(null, SELF_FULL), false);
  });

  it("canUnadmit: only with my confirmation", () => {
    assert.equal(canUnadmit(pendingRow({ admission: admissionInfo({ mine: true }) })), true);
    assert.equal(canUnadmit(pendingRow()), false);
    assert.equal(canUnadmit(extraRow({ admission: admissionInfo({ mine: true }) })), true);
    assert.equal(canUnadmit(null), false);
  });

  it("admittable keeps the order and skips the rest", () => {
    const a = pendingRow();
    const b = pendingRow({ id_full: "F".repeat(64), admission: admissionInfo({ mine: true }) });
    const reply = membersReply({ rows: [selfRow(), a, b, peerOneRow({ vote: foreignVote() })] });
    assert.deepEqual(admittable(reply, SELF_FULL), [parseMembers(reply).members[1]]);
    assert.deepEqual(admittable(null, SELF_FULL), []);
  });
});

describe("partitionRows: pending", () => {
  it("puts pending apart and keeps it out of active", () => {
    const reply = membersReply({ rows: [selfRow(), peerOneRow(), pendingRow()] });
    const peers = [{ id: "x", id_full: PENDING_FULL, name: null, online: false }, { id: "y", id_full: PEER_FULL, name: "p", online: true }];
    const parts = partitionRows(peers, reply, SELF_FULL);
    assert.deepEqual(parts.pending.map((m) => m.id_full), [PENDING_FULL]);
    assert.deepEqual(parts.active.map((p) => p.id_full).filter((id) => id !== SELF_FULL), [PEER_FULL]);
  });

  it("without a member list there are no pending rows", () => {
    assert.deepEqual(partitionRows([], null, "").pending, []);
  });
});

describe("rows: decoration and menu", () => {
  const held = extraRow({ admission: admissionInfo({ mine: true }) });

  it("a member held by confirmations shows the badge and the count", () => {
    const d = rowDecor({ id_full: EXTRA_FULL }, held, SELF_FULL);
    assert.deepEqual(d.badge, { text: "Waiting to join", tone: "warn" });
    assert.deepEqual(d.lines, ["Confirmed by 1 of 2"]);
  });

  it("a ban vote wins over the admission line", () => {
    const row = extraRow({ vote: foreignVote(), admission: admissionInfo() });
    assert.equal(rowDecor({ id_full: EXTRA_FULL }, row, SELF_FULL).badge.text, "Ban proposed");
  });

  it("the menu offers withdrawing only with my confirmation", () => {
    assert.ok(menuItems(held, { me: SELF_FULL }).some((i) => i.key === "unadmit" && i.label === "Withdraw confirmation"));
    const other = extraRow({ admission: admissionInfo({ mine: false }) });
    assert.ok(!menuItems(other, { me: SELF_FULL }).some((i) => i.key === "unadmit"));
  });

  it("a pending row's menu: withdraw (if mine) and block", () => {
    const mine = pendingRow({ admission: admissionInfo({ mine: true }) });
    assert.deepEqual(menuItems(mine, { me: SELF_FULL }).map((i) => i.key), ["unadmit", "block"]);
    assert.deepEqual(menuItems(pendingRow(), { me: SELF_FULL }).map((i) => i.key), ["block"]);
    assert.deepEqual(menuItems(pendingRow({ blocked_by_me: true }), { me: SELF_FULL }).map((i) => i.key), ["unblock"]);
  });
});

describe("admit dialog", () => {
  it("is a plain confirmation, not a danger one, and mentions the label", async () => {
    const { dialogSpec } = await import("./dialog-specs.ts");
    const spec = dialogSpec("admit", { name: "laptop", net: "home", label: "Anna's laptop" });
    assert.equal(spec.title, "Confirm laptop joining?");
    assert.equal(spec.action, "Confirm laptop");
    assert.equal(spec.danger, false);
    assert.ok(spec.body.includes('Invited as "Anna\'s laptop"'));
    assert.equal(dialogSpec("admit", { name: "laptop" }).body.length, 1);
  });
});
