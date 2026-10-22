/* The "Exit via…" list (`exit-list.ts`): candidates, labels, a single choice,
 * the "Check a site" result. The classic cases `14-home-exit-choice`, `-rows`, `14-home-exit` are carried over. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import {
  countryTag, exitCandidates, exitMetaText, findExitView, nextExitsOnChange, noPeersYet, otherExitSelector, pickOtherExit,
} from "./exit-list.ts";

const ids = (list) => list.map((item) => item.idFull);

describe("exitCandidates", () => {
  describe("choice is empty", () => {
    it("remembered [r] and exit member p — both rows, order: remembered, then member; not checked", () => {
      const list = exitCandidates({ peers: [{ id_full: "p", id: "pp", name: "kitchen", exit: true }] }, [], ["r"]);
      assert.deepEqual(ids(list), ["r", "p"]);
      assert.ok(list.every((item) => item.checked === false), "the remembered one and the exit member are not checked");
    });

    it("remembered without exit members — the row is not hidden", () => {
      const list = exitCandidates({ peers: [] }, [], ["r"]);
      assert.deepEqual(ids(list), ["r"]);
      assert.equal(list[0].checked, false);
    });

    it("remembered matches the exit member — one row", () => {
      const list = exitCandidates({ peers: [{ id_full: "p", id: "pp", name: "kitchen", exit: true }] }, [], ["p"]);
      assert.deepEqual(ids(list), ["p"]);
    });

    it("everything empty — the list is empty", () => assert.deepEqual(exitCandidates({ peers: [] }, [], []), []));
  });

  describe("there is a choice", () => {
    it("chosen [a] matches the exit member — one row checked: true; the own node is excluded", () => {
      const state = {
        peers: [
          { id_full: "a", id: "aa", name: "kitchen", exit: true },
          { id_full: "me", id: "mm", name: "me", exit: true, self: true },
        ],
      };
      const list = exitCandidates(state, ["a"], []);
      assert.deepEqual(ids(list), ["a"]);
      assert.equal(list[0].checked, true);
    });

    it("chosen ones go before the remembered one, no repeats", () => {
      const state = { peers: [{ id_full: "p", id: "pp", name: "x", exit: true }] };
      assert.deepEqual(ids(exitCandidates(state, ["b"], ["b", "r"])), ["b", "r", "p"]);
    });
  });

  describe("label and connection", () => {
    it("from route.exits without a peers row — the label and connection come from the route", () => {
      const state = { peers: [], route: { exits: [{ id_full: "x", id: "xx", name: "dacha", online: true, active: true }] } };
      const [item] = exitCandidates(state, ["x"], []);
      assert.equal(item.label, "dacha");
      assert.equal(item.online, true);
      assert.equal(item.active, true);
    });

    it("an unnamed member — \"member <id>\", not a bare id", () => {
      const [item] = exitCandidates({ peers: [{ id_full: "F", id: "ab12", name: null, online: true, exit: true }] }, [], []);
      assert.equal(item.label, "member ab12");
    });

    it("there is a name and no other member has it — the bare name, no id", () => {
      const [item] = exitCandidates({ peers: [{ id_full: "F", id: "ab12", name: "kitchen", exit: true }] }, [], []);
      assert.equal(item.label, "kitchen");
    });

    it("two members with the same name (case and edge spaces ignored) — both get \"name (id)\"", () => {
      const peers = [
        { id_full: "F", id: "ab12", name: "kitchen", exit: true },
        { id_full: "G", id: "cd34", name: " Kitchen ", exit: true },
        { id_full: "H", id: "ef56", name: "hall", exit: true },
      ];
      const labels = exitCandidates({ peers }, [], []).map((item) => item.label);
      assert.equal(labels[0], "kitchen (ab12)");
      assert.match(labels[1], /^\s*Kitchen\s*\(cd34\)$/);
      assert.equal(labels[2], "hall");
    });

    it("the own row with the same name counts as a duplicate too", () => {
      const peers = [
        { id_full: "F", id: "ab12", name: "kitchen", exit: true },
        { id_full: "ME", id: "mm00", name: "kitchen", self: true },
      ];
      const [item] = exitCandidates({ peers }, [], []);
      assert.equal(item.label, "kitchen (ab12)");
    });

    it("path, rtt_ms and country of the member go into the candidate; no fields — null", () => {
      const full = { id_full: "F", id: "ab12", name: "k", exit: true, online: true, path: "relay", rtt_ms: 30, country: "RU" };
      const bare = { id_full: "G", id: "cd34", name: "k", exit: true };
      const [a, b] = exitCandidates({ peers: [full, bare] }, [], []);
      assert.deepEqual([a.path, a.rttMs, a.country], ["relay", 30, "RU"]);
      assert.deepEqual([b.path, b.rttMs, b.country], [null, null, null]);
    });

    it("a remembered id with no data at all — an unnamed \"offline\" row", () => {
      const [item] = exitCandidates({ peers: [] }, [], ["r"]);
      assert.equal(item.label, "member r");
      assert.equal(item.online, false);
    });

    it("state === null — an empty list, no exception", () => assert.deepEqual(exitCandidates(null, [], []), []));
  });
});

describe("exitMetaText", () => {
  it("offline — no latency", () => {
    assert.equal(exitMetaText({ online: false, rttMs: 30, path: "direct" }), "offline");
  });

  it("online with latency — \"online · 30 ms\"", () => {
    assert.equal(exitMetaText({ online: true, rttMs: 30, path: null }), "online · 30 ms");
  });

  it("direct — \"online · direct · 30 ms\"", () => {
    assert.equal(exitMetaText({ online: true, rttMs: 30, path: "direct" }), "online · direct · 30 ms");
  });

  it("via relay server — \"online · via relay server · 30 ms\"", () => {
    assert.equal(exitMetaText({ online: true, rttMs: 30, path: "relay" }), "online · via relay server · 30 ms");
  });

  it("no latency — \"online\"; the words \"relay\" and \"offline\" are absent", () => {
    const text = exitMetaText({ online: true, rttMs: null, path: "relay" });
    assert.doesNotMatch(text, /\brelay\b(?! server)|offline/i);
    assert.equal(exitMetaText({ online: true, rttMs: null, path: null }), "online");
  });
});

describe("countryTag", () => {
  it("a known code — the country in words: \" · France\"", () => {
    assert.equal(countryTag({ country: "FR" }), " · France");
    assert.equal(countryTag({ country: "DE" }), " · Germany");
  });

  it("an unknown code (the name equals the code) — the code as it is", () => {
    assert.equal(countryTag({ country: "XX" }), " · XX");
  });

  it("a malformed code that Intl rejects — the code as it is, no exception", () => {
    assert.equal(countryTag({ country: "abc" }), " · abc");
  });

  it("no Intl.DisplayNames in the runtime — the code as it is", () => {
    vi.stubGlobal("Intl", { ...Intl, DisplayNames: undefined });
    assert.equal(countryTag({ country: "RU" }), " · RU");
  });

  it("no country — empty", () => assert.equal(countryTag({ country: null }), ""));
});

describe("nextExitsOnChange", () => {
  it("b switched on with a chosen — replaces", () => assert.deepEqual(nextExitsOnChange(["a"], "b", true), ["b"]));
  it("a switched on with an empty choice", () => assert.deepEqual(nextExitsOnChange([], "a", true), ["a"]));
  it("a switched off at [a] — empty", () => assert.deepEqual(nextExitsOnChange(["a"], "a", false), []));
  it("b switched off from the old [a, b] — a remains", () => assert.deepEqual(nextExitsOnChange(["a", "b"], "b", false), ["a"]));
  it("a switched off from the old [a, b] — b remains", () => assert.deepEqual(nextExitsOnChange(["a", "b"], "a", false), ["b"]));

  it("the result is never longer than one id", () => {
    const cases = [
      nextExitsOnChange(["a"], "b", true), nextExitsOnChange([], "a", true), nextExitsOnChange(["a"], "a", false),
      nextExitsOnChange(["a", "b"], "b", false), nextExitsOnChange(["a", "b"], "a", false),
    ];
    for (const result of cases) assert.ok(result.length <= 1, "got " + JSON.stringify(result));
  });
});

describe("findExitView", () => {
  const peers = [
    { id: "me", id_full: "me-full", name: "me", self: true },
    { id: "p", id_full: "p-full", name: "kitchen" },
    { id: "q", id_full: "q-full", name: "" },
  ];
  const withDomain = (entry) => ({ peers, domains: [{ name: "example.com", ...entry }] });

  describe("nothing to show", () => {
    it("no domain — kind none", () => {
      assert.equal(findExitView({ peers, domains: [] }, "").kind, "none");
      assert.equal(findExitView({ peers, domains: [] }, null).kind, "none");
    });
  });

  describe("waiting for a verdict", () => {
    it("the domain is not yet in the check list — checking", () => {
      const view = findExitView({ peers, domains: [] }, "example.com");
      assert.equal(view.kind, "checking");
      assert.equal(view.domain, "example.com");
    });

    it("the check is running (checking: true) — checking", () => {
      const state = withDomain({ checking: true, checks: { p: { verdict: "ok" } } });
      assert.equal(findExitView(state, "example.com").kind, "checking");
    });

    it("no verdicts yet — checking", () => {
      assert.equal(findExitView(withDomain({ checking: false, checks: {} }), "example.com").kind, "checking");
    });
  });

  describe("there are verdicts", () => {
    it("didn't open through anyone — empty", () => {
      const state = withDomain({ checking: false, checks: { p: { verdict: "blocked" } } });
      const view = findExitView(state, "example.com");
      assert.equal(view.kind, "empty");
      assert.equal(view.domain, "example.com");
    });

    it("the own node with ok is not offered, another one is", () => {
      const state = withDomain({ checking: false, checks: { me: { verdict: "ok" }, p: { verdict: "ok" } } });
      const view = findExitView(state, "example.com");
      assert.equal(view.kind, "ok");
      assert.deepEqual(view.items.map((i) => i.idFull), ["p-full"]);
      assert.equal(view.items[0].label, "kitchen");
    });

    it("two ok — both without the own node; an unnamed one — a bare id (a sentence phrase)", () => {
      const state = withDomain({ checking: false, checks: { me: { verdict: "ok" }, p: { verdict: "ok" }, q: { verdict: "ok" } } });
      const view = findExitView(state, "example.com");
      assert.deepEqual(view.items.map((i) => i.idFull), ["p-full", "q-full"]);
      assert.equal(view.items[1].label, "q");
    });

    it("only the own node with ok — empty", () => {
      const state = withDomain({ checking: false, checks: { me: { verdict: "ok" } } });
      assert.equal(findExitView(state, "example.com").kind, "empty");
    });
  });
});

describe("pickOtherExit", () => {
  const row = (idFull, online, checked) => ({ idFull, online, checked });

  it("the chosen one is offline, the second is online — the second", () => {
    assert.equal(pickOtherExit([row("b", false, true), row("c", true, false)]), "c");
  });

  it("the chosen one is online, doesn't fit, the first other online is taken", () => {
    assert.equal(pickOtherExit([row("b", true, true), row("c", false, false), row("d", true, false)]), "d");
  });

  it("no one online except the chosen one — \"As usual\" (an empty string)", () => {
    assert.equal(pickOtherExit([row("b", false, true), row("c", false, false)]), "");
    assert.equal(pickOtherExit([]), "");
  });
});

describe("otherExitSelector", () => {
  it("a member — by data-id, \"As usual\" — by #exit-direct", () => {
    const list = [{ idFull: "b", online: false, checked: true }, { idFull: "c", online: true, checked: false }];
    assert.equal(otherExitSelector(list), 'input.exit-choice[data-id="c"]');
    assert.equal(otherExitSelector([]), "#exit-direct");
  });
});

describe("noPeersYet", () => {
  const net = { id_hex: "aa", label: "home" };
  it("in a network with no peers or only self — true", () => {
    assert.equal(noPeersYet({ network: net, peers: [] }), true);
    assert.equal(noPeersYet({ network: net }), true);
    assert.equal(noPeersYet({ network: net, peers: [{ id_full: "s", id: "s", self: true }] }), true);
  });

  it("another member known, no network or no state — false", () => {
    assert.equal(noPeersYet({ network: net, peers: [{ id_full: "p", id: "p" }] }), false);
    assert.equal(noPeersYet({ peers: [] }), false);
    assert.equal(noPeersYet(null), false);
  });
});
