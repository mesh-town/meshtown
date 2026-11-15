/* The wizard state machine (`wizard-machine.ts`): every chain of up to three events from four starts keeps the
 * invariants of the shown page; the cases that used to lose or contradict an answer are pinned one by one. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { browserTiles, defaultBrowser, guideFor } from "./guide-content.ts";
import { applyEvent, ctxOf, freshCursor, guideOf, shownBrowser, shownOs, stepOf } from "./wizard-machine.ts";
import { screenOf, stripSteps, visibleRows, wizardPath } from "./wizard-steps.ts";

const STEPS = ["join", "scope", "os", "browser", "guide", "exit", "connect"];
const env = (over = {}) => ({ hasNet: true, inApp: false, proxyOn: true, mark: null, lan: "none", uaOs: "mac", uaBrowser: "chrome", ...over });
const state = (over = {}) => ({ scope: null, os: null, pick: null, ...freshCursor(), ...over });

const EVENTS = [
  ...["system", "browser"].map((value) => ({ kind: "scope", value })),
  ...["mac", "windows", "android"].map((value) => ({ kind: "os", value })),
  ...["safari", "chrome", "firefox", "edge", "other"].map((value) => ({ kind: "browser", value })),
  { kind: "next" }, { kind: "back" }, { kind: "skip" }, { kind: "toSystem" },
  ...STEPS.map((id) => ({ kind: "strip", id })),
];

/** One event, then the cursor is recorded as visited, the way the page does it. */
function run(s, e, events) {
  return events.reduce((cur, ev) => {
    const next = applyEvent(cur, e, ev);
    return { ...next, visited: [...new Set([...next.visited, stepOf(next, e)])] };
  }, s);
}

/** What the rows show, read apart from the machine: the pressed system and browser, hidden ones from the environment. */
function rowsPressed(s, e) {
  const rows = visibleRows(ctxOf(s, e), stepOf(s, e));
  const os = rows.includes("os") ? (s.os ?? e.uaOs) : e.uaOs;
  const ids = browserTiles(os).map(([id]) => id);
  const browser = rows.includes("browser") && ids.includes(s.pick) ? s.pick : defaultBrowser(os, e.uaBrowser);
  return { rows, os, browser };
}

const TOUCHED = { scope: ["scope"], os: ["os"], browser: ["pick"], toSystem: ["scope"] };

function check(before, after, e, ev) {
  const ctx = ctxOf(after, e);
  const step = stepOf(after, e);
  const label = JSON.stringify({ ev, before, after });
  assert.ok(wizardPath(ctx).includes(step), "the cursor is on the path " + label);
  const strip = stripSteps(ctx, step, new Set(after.visited));
  assert.equal(strip.filter((t) => t.state === "current").length, 1, "one current step " + label);
  const { rows, os, browser } = rowsPressed(after, e);
  if (rows.includes("browser")) assert.ok(browserTiles(shownOs(after, e)).some(([id]) => id === shownBrowser(after, e)), "the pressed browser has a tile " + label);
  if (rows.includes("guide") && screenOf(step) === "choices" && e.lan !== "router") assert.notEqual(after.scope, null, "the instruction has a scope " + label);
  const question = ctx.proxyOn && !after.skipped;
  if (e.lan === "router" && question) assert.ok(!rows.includes("scope") && !rows.includes("browser"), "no scope or browser row on a router " + label);
  if (rows.includes("guide")) assert.deepEqual(guideOf(after, e), guideFor(after.scope ?? "system", os, browser, e.lan), "the instruction follows the rows " + label);
  for (const key of ["scope", "os", "pick"]) {
    if (!(TOUCHED[ev.kind] ?? []).includes(key)) assert.equal(after[key], before[key], key + " is not touched by " + ev.kind);
  }
}

function walk(s, e, depth, seen) {
  if (depth === 0) return;
  for (const ev of EVENTS) {
    const next = run(s, e, [ev]);
    check(s, next, e, ev);
    seen.count += 1;
    walk(next, e, depth - 1, seen);
  }
}

describe("wizard machine: chains of events", () => {
  const starts = [
    ["a clean start", state(), env()],
    ["a remembered scope", state({ scope: "system" }), env()],
    ["a remembered browser scope and pick", state({ scope: "browser", pick: "safari" }), env()],
    ["the proxy off", state(), env({ proxyOn: false })],
    ["a router", state(), env({ lan: "router" })],
  ];
  for (const [name, s, e] of starts) {
    it(`given ${name}, then every chain of three events keeps the invariants`, () => {
      const seen = { count: 0 };
      walk(s, e, 3, seen);
      assert.ok(seen.count > EVENTS.length ** 3);
    });
  }
});

describe("wizard machine: the cases of the dossier", () => {
  it("given one browser, Safari, then Set up for all apps, then the instruction is for all apps and has no browser row", () => {
    const past = env({ mark: "browser" });
    const s = run(state(), past, [{ kind: "scope", value: "browser" }, { kind: "browser", value: "safari" }, { kind: "toSystem" }]);
    assert.equal(guideOf(s, past).title, "Set up all apps on macOS");
    assert.equal(stepOf(s, past), "guide");
    assert.ok(!visibleRows(ctxOf(s, env()), "guide").includes("browser"));
  });

  it("given Safari picked on macOS, then Windows: Chrome is pressed, the pick stays, and macOS brings Safari back", () => {
    const answers = state({ scope: "browser", pick: "safari" });
    const win = env({ uaOs: "windows" });
    assert.equal(shownBrowser(answers, win), "chrome");
    assert.equal(answers.pick, "safari");
    assert.equal(shownBrowser(answers, env({ uaOs: "mac" })), "safari");
  });

  it("given one browser, then the system answer is not shown: the detected system is", () => {
    const answers = state({ scope: "browser", os: "windows" });
    assert.equal(shownOs(answers, env()), "mac");
    assert.equal(shownOs({ ...answers, scope: "system" }, env()), "windows");
  });

  it("given remembered answers and a fresh cursor, then the cursor is exit, and past it the instruction with the picked browser", () => {
    const answers = state({ scope: "browser", pick: "firefox" });
    const s = { ...answers, ...freshCursor() };
    assert.equal(stepOf(s, env()), "exit", "no mark: the exit step comes first");
    assert.equal(stepOf(s, env({ mark: "browser" })), "guide");
    assert.equal(shownBrowser(s, env()), "firefox");
  });

  it("given the proxy off and on again, then the cursor is where it was", () => {
    const past = env({ mark: "browser" });
    const s = run(state(), past, [{ kind: "scope", value: "system" }]);
    assert.equal(stepOf(s, past), "guide");
    assert.equal(stepOf(s, env({ proxyOn: false, mark: "browser" })), "scope");
    assert.equal(stepOf(s, past), "guide");
  });

  it("given a pick that the shown system has no tile for, then the instruction is the system default's", () => {
    const answers = state({ scope: "browser", pick: "safari" });
    const g = guideOf(answers, env({ uaOs: "windows", uaBrowser: "edge" }));
    assert.equal(g.title, "Set up Edge");
  });
});

describe("wizard machine: the exit step comes first", () => {
  it("given the exit step, then Next opens the first question and settles past the answered rows", () => {
    const e = env();
    const first = run(state({ scope: "system" }), e, [{ kind: "next" }]);
    assert.equal(stepOf(first, e), "guide");
    const fresh = run(state(), e, [{ kind: "next" }]);
    assert.equal(stepOf(fresh, e), "scope");
  });

  it("given the questions page, then Back from the first question goes to exit and the strip returns to the rows", () => {
    const e = env({ mark: "browser" });
    const s = run(state({ scope: "system", visited: ["guide"] }), e, [{ kind: "strip", id: "exit" }]);
    assert.equal(stepOf(s, e), "exit");
    const back = run(s, e, [{ kind: "strip", id: "guide" }]);
    assert.equal(stepOf(back, e), "guide");
  });

  it("given the skipped question, then the path ends on the scope step", () => {
    const e = env({ mark: "browser" });
    const s = run(state(), e, [{ kind: "skip" }]);
    assert.deepEqual(wizardPath(ctxOf(s, e)), ["join", "exit", "scope"]);
    assert.equal(stepOf(s, e), "scope");
  });
});

describe("wizard machine: on a router", () => {
  const router = env({ lan: "router" });

  it("given a router and the system iOS, then the instruction is for connecting an iPhone to the router", () => {
    const s = run(state({ scope: "browser" }), router, [{ kind: "os", value: "ios" }]);
    assert.equal(shownOs(s, router), "ios");
    assert.equal(guideOf(s, router).title, "Connect an iPhone or iPad to the router");
  });

  it("given a router and a fresh cursor, then the cursor is exit, and after the exit step the instruction", () => {
    assert.equal(stepOf(state(), router), "exit");
    assert.equal(stepOf(state(), env({ lan: "router", mark: "browser" })), "guide");
  });

  it("given a router, then the detected system is shown until one is chosen", () => {
    assert.equal(shownOs(state({ scope: "browser" }), router), "mac");
  });
});
