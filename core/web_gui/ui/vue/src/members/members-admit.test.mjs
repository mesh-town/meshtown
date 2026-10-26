/* Join requests ("admission") in the members logic: parsing, errors, the admit rule, grouping and the menu. Contract:
 * `GET /api/members` rows may have status "pending" and an `admission` object `{ label }`; a reply may carry
 * `me_pending` / `me_admission` and `counts.pending`; an old reply without them still parses. An admin admits alone:
 * there is no count of confirmations and nothing to withdraw. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { memberErrorText, parseMembers, postMemberAction } from "./members-api.ts";
import { admittable, canAdmit, menuItems, partitionRows, rowDecor } from "./members-view.ts";
import {
  EXTRA_FULL,
  admissionInfo,
  extraRow,
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
      rows: [selfRow(), pendingRow({ admission: admissionInfo({ label }) })],
      me_pending: true,
      me_admission: admissionInfo(),
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
    const raw = membersReply({ rows: [pendingRow({ admission: { label: "  " } }), extraRow({ admission: "x" })] });
    const parsed = parseMembers(raw);
    assert.equal(parsed.members[0].admission.label, null);
    assert.equal(parsed.members[1].admission, null);
  });
});

describe("admit", () => {
  it("post to the action path with the id", async () => {
    const calls = [];
    const api = async (method, path, body) => (calls.push({ method, path, body }), { ...membersReply(), changed: true });
    const out = await postMemberAction(api, "admit", PENDING_FULL);
    assert.deepEqual(calls[0], { method: "POST", path: "/api/members/admit", body: { id: PENDING_FULL } });
    assert.equal(out.changed, true);
  });

  it("a refusal has a human reason", () => {
    const err = (code) => new ApiError(409, code, code);
    assert.equal(memberErrorText(err("not_pending")), "Couldn't update the member list: this member isn't waiting for confirmation");
  });
});

describe("canAdmit / admittable", () => {
  it("canAdmit: a pending row of someone else", () => {
    assert.equal(canAdmit(pendingRow(), SELF_FULL), true);
  });

  it("canAdmit is false for a member, an excluded row, myself and null", () => {
    assert.equal(canAdmit(peerOneRow(), SELF_FULL), false);
    assert.equal(canAdmit(extraRow({ status: "excluded" }), SELF_FULL), false);
    assert.equal(canAdmit(pendingRow({ id_full: SELF_FULL }), SELF_FULL), false);
    assert.equal(canAdmit(null, SELF_FULL), false);
  });

  it("admittable keeps the order and skips the rest", () => {
    const a = pendingRow();
    const b = pendingRow({ id_full: "F".repeat(64) });
    const reply = membersReply({ rows: [selfRow(), a, peerOneRow(), b] });
    assert.deepEqual(admittable(reply, SELF_FULL).map((m) => m.id_full), [a.id_full, b.id_full]);
    assert.deepEqual(admittable(null, SELF_FULL), []);
  });
});

describe("partitionRows: pending", () => {
  it("puts pending apart and keeps it out of active", () => {
    const reply = membersReply({ rows: [selfRow(), peerOneRow(), pendingRow()] });
    const peers = [{ id: "x", id_full: PENDING_FULL, name: null, online: false }, { id: "y", id_full: PEER_FULL, name: "p", online: true }];
    const parts = partitionRows(peers, reply);
    assert.deepEqual(parts.pending.map((m) => m.id_full), [PENDING_FULL]);
    assert.deepEqual(parts.active.map((p) => p.id_full).filter((id) => id !== SELF_FULL), [PEER_FULL]);
  });

  it("without a member list there are no pending rows", () => {
    assert.deepEqual(partitionRows([], null).pending, []);
  });
});

describe("rows: decoration and menu", () => {
  it("a member row with an admission shows nothing extra", () => {
    const row = extraRow({ admission: admissionInfo() });
    assert.deepEqual(rowDecor({ id_full: EXTRA_FULL }, row, SELF_FULL), { badge: null, lines: [], mainAction: null });
  });

  it("a pending row's menu: block, or unblock when blocked; nothing to withdraw", () => {
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
    assert.ok(spec.body[0].endsWith("it joins right away."), spec.body[0]);
  });
});
