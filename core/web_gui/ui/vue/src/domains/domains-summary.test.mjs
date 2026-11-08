import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { exitPath, siteSummary, viaHead } from "./domains-summary.ts";

const peer = (id, over = {}) => ({ id, id_full: id + "-full", name: "exit-" + id, online: true, exit: true, ...over });
const net = (slot, peers) => ({ slot, label: "net-" + slot, selfId: "me", peers });
const ok = (age_sec = 120) => ({ verdict: "ok", reason: "", age_sec });
const bad = (verdict = "network_blocked", age_sec = 300) => ({ verdict, reason: "", age_sec });
const site = (byNet, checking = false) => ({
  name: "a.org",
  checking,
  byNet: Object.fromEntries(Object.entries(byNet).map(([slot, checks]) => [slot, { name: "a.org", checks, checking }])),
});
const via = (p, over = {}) => ({ id: p.id, id_full: p.id_full, name: p.name, online: true, exit: true, country: null, active: true, ...over });

const a = peer("aa");
const b = peer("bb");
const c = peer("cc");

describe("siteSummary", () => {
  it("active exit opens — opens, a counter of the other exits, no button", () => {
    const s = siteSummary(site({ main: { me: bad(), aa: ok(), bb: ok(), cc: ok() } }), [net("main", [a, b, c])], "main", [via(a)]);
    assert.equal(s.kind, "opens");
    assert.equal(s.mark.text, "opens");
    assert.equal(s.age, "2 min ago");
    assert.deepEqual(s.others, { lead: "also via", more: "2 more" });
    assert.equal(s.button, null);
  });

  it("active exit blocked, an alternative in the proxy network — a button on it", () => {
    const s = siteSummary(site({ main: { me: bad(), aa: bad(), bb: ok() } }), [net("main", [a, b])], "main", [via(a)]);
    assert.equal(s.kind, "blocked");
    assert.equal(s.mark.text, "blocked by provider");
    assert.deepEqual(s.others, { lead: "opens via exit-bb", more: "" });
    assert.deepEqual(s.button, { idFull: "bb-full", name: "exit-bb" });
  });

  it("an alternative only in another network — text without a button", () => {
    const nets = [net("main", [a]), net("work", [b])];
    const s = siteSummary(site({ main: { me: bad(), aa: bad() }, work: { me: bad(), bb: ok() } }), nets, "main", [via(a)]);
    assert.deepEqual(s.others, { lead: "opens via exit-bb", more: "" });
    assert.equal(s.button, null);
  });

  it("the first chosen exit is offline, a spare is active — the path goes through the spare", () => {
    const exits = [via(a, { active: false, online: false }), via(b)];
    const nets = [net("main", [b, c])];
    assert.equal(exitPath(nets, "main", exits).id, "bb");
    const s = siteSummary(site({ main: { me: bad(), bb: ok(), cc: ok() } }), nets, "main", exits);
    assert.equal(s.kind, "opens");
    assert.deepEqual(s.others, { lead: "also via", more: "1 more" });
    assert.equal(viaHead(exitPath(nets, "main", exits)).text, "Via exit-bb");
  });

  it("chosen but not active, offline — exit-offline with the alternatives", () => {
    const s = siteSummary(site({ main: { me: bad(), bb: ok() } }), [net("main", [b])], "main", [via(a, { active: false, online: false })]);
    assert.equal(s.kind, "exit-offline");
    assert.equal(s.mark, null);
    assert.deepEqual(s.button, { idFull: "bb-full", name: "exit-bb" });
  });

  it("chosen but not active, online — exit-not-sharing", () => {
    const s = siteSummary(site({ main: { me: ok() } }), [net("main", [b])], "main", [via(a, { active: false, online: true })]);
    assert.equal(s.kind, "exit-not-sharing");
  });

  it("no exit chosen, down for you while a member opens it — doesn't open from here", () => {
    const nets = [net("main", [a])];
    const s = siteSummary(site({ main: { me: bad("down"), aa: ok() } }), nets, "main", []);
    assert.equal(s.kind, "blocked");
    assert.equal(s.mark.text, "doesn't open from here");
    assert.deepEqual(viaHead(exitPath(nets, "main", [])), { text: "For you", sub: "no exit chosen" });
  });

  it("checking and a missing record", () => {
    const nets = [net("main", [a])];
    assert.equal(siteSummary(site({ main: { me: ok() } }, true), nets, "main", [via(a)]).kind, "checking");
    const s = siteSummary(site({ main: { me: ok() } }), nets, "main", [via(a)]);
    assert.equal(s.kind, "unknown");
    assert.equal(s.mark.text, "—");
  });

  it("same-named members in two networks are told apart by network", () => {
    const x = peer("xx", { name: "twin" });
    const y = peer("yy", { name: "twin" });
    const nets = [net("main", [a, x]), net("work", [y])];
    const s = siteSummary(site({ main: { me: bad(), aa: bad(), xx: ok() }, work: { me: bad(), yy: ok() } }), nets, "main", [via(a)]);
    assert.deepEqual(s.others, { lead: "opens via twin and", more: "1 more" });
    assert.equal(s.button.idFull, "xx-full");
  });

  it("the counter is a separate part: three alternatives, open or blocked", () => {
    const d = (me) => site({ main: { me: bad(), aa: me, bb: ok(), cc: ok(), dd: ok() } });
    const nets = [net("main", [a, b, c, peer("dd")])];
    assert.deepEqual(siteSummary(d(ok()), nets, "main", [via(a)]).others, { lead: "also via", more: "3 more" });
    assert.deepEqual(siteSummary(d(bad()), nets, "main", [via(a)]).others, { lead: "opens via exit-bb and", more: "2 more" });
  });

  it("no alternatives or not checked yet — the whole text in lead, no counter", () => {
    const none = siteSummary(site({ main: { me: bad(), aa: bad() } }), [net("main", [a])], "main", [via(a)]);
    assert.equal(none.others.more, "");
    assert.equal(none.others.lead, "no other exit opens it");
    const fresh = siteSummary(site({}), [net("main", [a])], "main", [via(a)]);
    assert.equal(fresh.others.more, "");
  });
});
