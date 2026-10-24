/* The one invite form without Vue (`invite-join-actions.ts`): where a pasted string goes, how the answer is read, the note under the field. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createJoinForm } from "./join-actions.ts";
import { sendInvite, placeNote, isInvite } from "./invite-join-actions.ts";
import { ADD_NET_LIMIT } from "./add-net-texts.ts";
import { NOTE_BANNED_PEER, NOTE_EXCLUDED_MANAGED, NOTE_HERE, NOTE_NEW_AT_LIMIT, NOTE_REMOVED, NOTE_THIS_NET } from "./invite-join-texts.ts";

const INVITE = "mesh-invite2-abc";
const WORD_KEY = "mesh1-abc";
const JOIN = "/api/network/join";

// `table` — "METHOD path" → reply, or a function (it may throw an ApiError).
function setup(table = {}) {
  const calls = [];
  const api = async (method, path, body) => {
    calls.push([method, path, structuredClone(body)]);
    const entry = table[method + " " + path];
    if (entry === undefined) throw new Error("unexpected request " + method + " " + path);
    return typeof entry === "function" ? entry(body) : entry;
  };
  const deps = { api, storage: null, setTimer: () => 0 };
  const form = createJoinForm();
  return { form, calls, deps, paths: () => calls.map((c) => c[1]) };
}
const send = (w, text, ctx) => {
  w.form.inviteInput = text;
  return sendInvite(w.form, ctx, w.deps);
};
const refuse = (status, code, body = {}) => () => { throw new ApiError(status, code, code, body); };
const CONTEXTS = [{ kind: "home" }, { kind: "add" }, { kind: "rejoin", slot: "neta22" }];

describe("sendInvite > an invite", () => {
  for (const ctx of CONTEXTS) {
    it(`${ctx.kind}: one POST /api/network/join {key}, no slot address, no /api/nets`, async () => {
      const w = setup({ ["POST " + JOIN]: { label: "family", restart: true, slot: "x1y2z3" } });
      assert.equal(await send(w, "  " + INVITE + " ", ctx), "joined");
      assert.deepEqual(w.calls.map((c) => [c[0], c[1], c[2]]), [["POST", JOIN, { key: INVITE }]]);
      assert.equal(w.form.slot, "x1y2z3");
      assert.equal(w.form.inviteInput, "");
    });
  }

  it("an answer without a slot (an older node) — `main`", async () => {
    const w = setup({ ["POST " + JOIN]: { label: "family", restart: true } });
    await send(w, INVITE, { kind: "home" });
    assert.equal(w.form.slot, "main");
  });

  it("waiting — the status names the network", async () => {
    const w = setup({ ["POST " + JOIN]: { label: "family", restart: false, waiting: true, slot: "neta22" } });
    assert.equal(await send(w, INVITE, { kind: "add" }), "waiting");
    assert.equal(w.form.slot, "neta22");
    assert.ok(w.form.status.includes('"family"'));
  });

  it("409 net_already_added — \"here\", the slot to open, no error", async () => {
    const w = setup({ ["POST " + JOIN]: refuse(409, "net_already_added", { slot: "neta22", label: "family" }) });
    assert.equal(await send(w, INVITE, { kind: "add" }), "here");
    assert.equal(w.form.dupSlot, "neta22");
    assert.equal(w.form.inviteError, null);
  });

  it("409 too_many_nets — \"limit\" and the limit text", async () => {
    const w = setup({ ["POST " + JOIN]: refuse(409, "too_many_nets") });
    assert.equal(await send(w, INVITE, { kind: "add" }), "limit");
    assert.equal(w.form.inviteError.message, ADD_NET_LIMIT);
  });

  it("422 with a word — \"failed\", the word kept", async () => {
    const w = setup({ ["POST " + JOIN]: refuse(422, "invalid_key", { message: 'Bad "zebra"', word: "zebra" }) });
    assert.equal(await send(w, INVITE, { kind: "home" }), "failed");
    assert.deepEqual(w.form.inviteError, { message: 'Bad "zebra"', word: "zebra" });
  });

  it("409 busy — \"failed\" with the node's reason", async () => {
    const w = setup({ ["POST " + JOIN]: refuse(409, "busy", { message: "the node is busy" }) });
    assert.equal(await send(w, INVITE, { kind: "home" }), "failed");
    assert.ok(w.form.inviteError.message.includes("busy"));
  });
});

describe("sendInvite > a word key", () => {
  it("add: a slot of its own — /api/nets, then the address of that slot", async () => {
    const w = setup({
      "POST /api/nets": { slot: "neta22", fresh: true },
      "POST /api/net/neta22/network/join": { label: "family", restart: true },
    });
    assert.equal(await send(w, WORD_KEY, { kind: "add" }), "joined");
    assert.deepEqual(w.paths(), ["/api/nets", "/api/net/neta22/network/join"]);
    assert.equal(w.form.slot, "neta22");
  });

  it("home: /api/network/join", async () => {
    const w = setup({ ["POST " + JOIN]: { label: "family", restart: true } });
    await send(w, WORD_KEY, { kind: "home" });
    assert.deepEqual(w.paths(), [JOIN]);
  });

  it("rejoin: the address of the tab's slot", async () => {
    const w = setup({ "POST /api/net/neta22/network/join": { label: "family", restart: true } });
    assert.equal(await send(w, WORD_KEY, { kind: "rejoin", slot: "neta22" }), "joined");
    assert.deepEqual(w.paths(), ["/api/net/neta22/network/join"]);
    assert.equal(w.form.slot, "neta22");
  });
});

describe("sendInvite > guards", () => {
  it("empty field — \"empty\", the error is on the form, no requests", async () => {
    const w = setup();
    assert.equal(await send(w, "   ", { kind: "home" }), "empty");
    assert.ok(w.form.inviteError.message.length > 0);
    assert.equal(w.calls.length, 0);
  });

  it("a second call while the first is in flight — \"skipped\", one request", async () => {
    let release = () => {};
    const gate = new Promise((resolve) => { release = resolve; });
    const w = setup({ ["POST " + JOIN]: async () => { await gate; return { label: null, restart: true }; } });
    w.form.inviteInput = INVITE;
    const first = sendInvite(w.form, { kind: "home" }, w.deps);
    assert.equal(await sendInvite(w.form, { kind: "home" }, w.deps), "skipped");
    release();
    assert.equal(await first, "joined");
    assert.equal(w.calls.length, 1);
  });

  it("isInvite looks at the trimmed start", () => {
    assert.equal(isInvite("  mesh-invite2-x"), true);
    assert.equal(isInvite("mesh1-x"), false);
  });
});

describe("placeNote", () => {
  const nets = [
    { id: "neta22", label: "family", id_hex: "aa", me_excluded: true },
    { id: "netb33", label: "work", id_hex: "bb", me_excluded: false },
  ];
  const reply = (over) => ({ label: "family", kind: "Peer", id_hex: "aa", ...over });

  it("here — the name of the slot", () => {
    assert.equal(placeNote(reply({ place: "here", slot: "netb33" }), nets), NOTE_HERE("work"));
  });
  it("return of a removed computer", () => {
    assert.equal(placeNote(reply({ place: "return", slot: "netb33" }), nets), NOTE_REMOVED("work"));
  });
  it("return of an excluded computer: Managed and Peer", () => {
    assert.equal(placeNote(reply({ place: "return", slot: "neta22", kind: "Managed" }), nets), NOTE_EXCLUDED_MANAGED("family"));
    assert.equal(placeNote(reply({ place: "return", slot: "neta22" }), nets), NOTE_BANNED_PEER("family"));
  });
  it("new, no place, a key — nothing; new at the limit — the limit note", () => {
    assert.equal(placeNote(reply({ place: "new" }), nets), "");
    assert.equal(placeNote(reply({}), nets), "");
    assert.equal(placeNote(reply({ place: "new" }), nets, true), NOTE_NEW_AT_LIMIT);
  });
  it("a slot that is not in the list — the node's label, then a placeholder without quotes", () => {
    assert.equal(placeNote(reply({ place: "here", slot: "nope99" }), nets), NOTE_HERE("family"));
    assert.equal(placeNote(reply({ place: "here", slot: "nope99", label: null }), nets), "You're already in " + NOTE_THIS_NET + ".");
  });
});
