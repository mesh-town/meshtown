/* Pure logic of the "Site checks" section (`domains-view.ts`): the table rows, marks
 * and actions. */

import { agoText } from "../common/words.ts";
import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  LEGEND, VERDICT_TEXT, VERDICT_BADGE, addFailedText, checkAllLabel, checkTitle,
  domainCells,
  otherPeers, siteNets, rowsBusy, verdictView,
} from "./domains-view.ts";

const me = { id: "me", id_full: "me-full", name: "me", self: true };
const good = (id, extra = {}) => ({ id, id_full: id + "-full", name: "node-" + id, exit: true, online: true, ...extra });
const dom = (checks) => ({ name: "a.org", checks });
const ok = { verdict: "ok" };
const bad = { verdict: "network_blocked" };

describe("agoText / checkTitle", () => {
  it("scale boundary: 59 s — \"just now\", 60 s — \"1 min ago\", 7200 s — \"2 h ago\"", () => {
    assert.equal(agoText(59), "just now");
    assert.equal(agoText(60), "1 min ago");
    assert.equal(agoText(7200), "2 h ago");
  });
  it("null or not a number — empty string", () => {
    assert.equal(agoText(null), "");
    assert.equal(agoText(3 * 86400), "3 d ago");
    assert.equal(agoText("x"), "");
  });
  it("with a reason — \"reason · checked N ago\"", () => {
    assert.equal(checkTitle({ reason: "TLS connection reset", age_sec: 120 }), "TLS connection reset · checked 2 min ago");
  });
  it("empty reason — only the age; no record — \"not checked yet\"", () => {
    assert.equal(checkTitle({ reason: "", age_sec: 5 }), "checked just now");
    assert.equal(checkTitle(null), "not checked yet");
  });
});

describe("verdictView — mark labels", () => {
  it("ok — \"opens\", green class", () => {
    assert.deepEqual(verdictView({ verdict: "ok", reason: "", age_sec: 1 }), {
      cls: "badge-ok", text: "opens", title: "checked just now",
    });
  });
  it("network_blocked — \"blocked by provider\"", () => {
    const v = verdictView({ verdict: "network_blocked" });
    assert.equal(v.text, "blocked by provider");
    assert.equal(v.cls, "badge-cut");
  });
  it("no record — \"—\" and a grey class", () => {
    const v = verdictView(null);
    assert.equal(v.text, "—");
    assert.equal(v.cls, "badge-none");
    assert.equal(v.title, "not checked yet");
  });
  it("unknown verdict — \"result unknown\", the code in title, grey class", () => {
    const v = verdictView({ verdict: "new_one" });
    assert.equal(v.text, "result unknown");
    assert.equal(v.title, "check code: new_one");
    assert.equal(v.cls, "badge-none");
  });
  it("checkTitle: no reason — only \"checked …\"; the reason is kept", () => {
    assert.equal(checkTitle({ age_sec: 3 * 86400 }), "checked 3 d ago");
    assert.equal(checkTitle({ reason: "TLS", age_sec: 61 }), "TLS · checked 1 min ago");
  });
});

describe("table columns", () => {
  it("siteNets keeps only exit members", () => {
    const store = { state: { node: { id: "me" }, peers: [me, { ...good("p1"), exit: true }, { ...good("p2"), exit: false }, { ...good("p3"), online: false, exit: undefined }] }, lead: "main", nets: [] };
    assert.deepEqual(siteNets(store)[0].peers.map((p) => p.id), ["p1"]);
  });

  it("otherPeers — members without yourself, in snapshot order", () => {
    assert.deepEqual(otherPeers({ peers: [me, good("p1"), good("p2")] }).map((p) => p.id), ["p1", "p2"]);
    assert.deepEqual(otherPeers(null), []);
  });
});

describe("\"Check all\"", () => {
  it("rowsBusy — there is a row in checking", () => {
    assert.equal(rowsBusy([{ name: "a.org", checking: true }]), true);
    assert.equal(rowsBusy([{ name: "a.org", checking: false }]), false);
    assert.equal(rowsBusy(null), false);
  });
  it("button label: busy — \"Checking…\", otherwise \"Check all\"", () => {
    assert.equal(checkAllLabel(true), "Checking…");
    assert.equal(checkAllLabel(false), "Check all");
  });
});

describe("LEGEND — badge = VERDICT_TEXT verbatim", () => {
  it("a line per verdict, \"doesn't open from here\" before \"site is down\", plus \"not checked yet\"", () => {
    const keys = Object.keys(VERDICT_TEXT);
    assert.equal(LEGEND.length, keys.length + 2);
    const plain = LEGEND.filter((l) => l.text !== "doesn't open from here");
    keys.forEach((k, i) => {
      assert.equal(plain[i].text, VERDICT_TEXT[k]);
      assert.equal(plain[i].cls, VERDICT_BADGE[k]);
    });
    const hereAt = LEGEND.findIndex((l) => l.text === "doesn't open from here");
    assert.equal(LEGEND[hereAt].cls, VERDICT_BADGE.network_blocked);
    assert.equal(LEGEND[hereAt + 1].text, VERDICT_TEXT.down);
    const last = LEGEND[LEGEND.length - 1];
    assert.deepEqual([last.text, last.cls, last.note], ["—", "badge-none", "not checked yet"]);
  });
});

describe("addFailedText", () => {
  it("with a reason — \"Couldn't add the site: …\"; without — a hint about the address", () => {
    assert.equal(addFailedText({ body: { message: "Bad name" } }), "Couldn't add the site: bad name");
    assert.equal(addFailedText({}), "Couldn't add the site — make sure the address looks like example.com");
  });
});

describe("a network without members", () => {
  const empty = (slot) => ({ slot, label: slot, selfId: "me", peers: [] });
  const merged = { name: "a.org", checking: false, byNet: {} };

  it("one network — no placeholder cell", () => {
    assert.deepEqual(domainCells(merged, [empty("main")]).map((c) => c.kind), ["you"]);
  });

  it("two networks — each empty network gets a placeholder column", () => {
    const nets = [empty("main"), empty("b")];
    assert.deepEqual(domainCells(merged, nets).map((c) => c.kind), ["you", "empty", "empty"]);
  });
});

describe("you cell: doesn't open from here", () => {
  const net = (slot, peers) => ({ slot, label: slot, selfId: "me", peers });
  const row = (slot, checks) => ({ name: "a.org", checking: false, byNet: { [slot]: { name: "a.org", checks } } });
  const peers = [{ id: "p1", name: "p1" }];
  const youMark = (m, nets) => domainCells(m, nets)[0].mark;

  it("you down and a member ok shows doesn't open from here", () => {
    const m = youMark(row("main", { me: { verdict: "down" }, p1: ok }), [net("main", peers)]);
    assert.equal(m.text, "doesn't open from here");
    assert.equal(m.cls, VERDICT_BADGE.network_blocked);
    assert.ok(m.title.startsWith("The site answers other members but not you"));
  });
  it("you down and no member ok stays site is down", () => {
    const m = youMark(row("main", { me: { verdict: "down" }, p1: { verdict: "down" } }), [net("main", peers)]);
    assert.equal(m.text, VERDICT_TEXT.down);
    assert.equal(m.cls, VERDICT_BADGE.down);
  });
  it("you ok is unchanged when members are down", () => {
    const m = youMark(row("main", { me: ok, p1: { verdict: "down" } }), [net("main", peers)]);
    assert.equal(m.text, VERDICT_TEXT.ok);
  });
});
