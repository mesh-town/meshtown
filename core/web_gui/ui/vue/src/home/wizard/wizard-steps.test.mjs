/* The step graph of the setup wizard (`wizard-steps.ts`): the path by the answers, the start step, the
 * neighbours, the progress counter, the kind of the forward button, the setup mark and the joined line. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  isPhoneOs, joinedLineOn, markOnEnter, navKind, nextStep, prevStep, reanchor, screenOf, settleStep, startStep, stepProgress, stripSteps, validStep, visibleRows, wizardPath,
} from "./wizard-steps.ts";

const ctx = (over = {}) => ({ hasNet: true, inApp: false, proxyOn: true, scope: "system", os: "mac", skipped: false, ...over });

describe("wizardPath", () => {
  describe("in the app", () => {
    it("given the app page, then the path is join, exit, connect", () => {
      assert.deepEqual(wizardPath(ctx({ inApp: true })), ["join", "exit", "connect"]);
    });
  });

  describe("proxy is off", () => {
    it("given the proxy is off, then the path is join, scope, exit", () => {
      assert.deepEqual(wizardPath(ctx({ proxyOn: false })), ["join", "scope", "exit"]);
    });
  });

  describe("the scope question was skipped", () => {
    it("given skipped, then the path is join, scope, exit", () => {
      assert.deepEqual(wizardPath(ctx({ scope: null, skipped: true })), ["join", "scope", "exit"]);
    });
  });

  describe("scope is the whole system", () => {
    it("given system on macOS, then the path has os, browser and guide", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "system", os: "mac" })), ["join", "scope", "os", "browser", "guide", "exit"]);
    });

    it("given no answer yet on macOS, then the path is the recommended one with six steps", () => {
      assert.deepEqual(wizardPath(ctx({ scope: null, os: "mac" })), ["join", "scope", "os", "browser", "guide", "exit"]);
    });

    it("given system on Android, then there is no browser step", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "system", os: "android" })), ["join", "scope", "os", "guide", "exit"]);
    });
  });

  describe("scope is one browser", () => {
    it("given browser on Linux, then there is no os step", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "browser", os: "linux" })), ["join", "scope", "browser", "guide", "exit"]);
    });

    it("given browser on iOS, then there are neither os nor browser steps", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "browser", os: "ios" })), ["join", "scope", "guide", "exit"]);
    });
  });
});

describe("startStep", () => {
  it("given no network, then the start is join", () => {
    assert.equal(startStep(ctx({ hasNet: false }), null), "join");
  });

  it("given the app page, then the start is exit", () => {
    assert.equal(startStep(ctx({ inApp: true }), null), "exit");
  });

  it("given the exit mark, then the start is exit", () => {
    assert.equal(startStep(ctx(), "exit"), "exit");
  });

  it("given the browser mark and no scope answer, then the start is scope", () => {
    assert.equal(startStep(ctx({ scope: null }), "browser"), "scope");
  });

  it("given no mark and no scope answer, then the start is scope", () => {
    assert.equal(startStep(ctx({ scope: null }), null), "scope");
  });

  it("given an unknown mark, then it is treated as no mark and the start is scope", () => {
    assert.equal(startStep(ctx({ scope: null }), "garbage"), "scope");
  });

  it("given a remembered scope, then the answered rows are passed and the start is the instruction", () => {
    assert.equal(startStep(ctx(), null), "guide");
  });

  it("given the proxy off, then the start is scope even with a remembered answer", () => {
    assert.equal(startStep(ctx({ proxyOn: false }), null), "scope");
  });
});

describe("validStep", () => {
  it("given the current step is on the path, then it stays", () => {
    assert.equal(validStep(ctx(), "guide", null), "guide");
  });

  it("given the join step after the network appeared, then the wizard moves to the start step", () => {
    assert.equal(validStep(ctx({ scope: null }), "join", null), "scope");
  });

  it("given the join step and the exit mark after the network appeared, then it moves to exit", () => {
    assert.equal(validStep(ctx(), "join", "exit"), "exit");
  });

  it("given the browser step after the proxy was turned off, then it falls back to scope", () => {
    assert.equal(validStep(ctx({ proxyOn: false }), "browser", null), "scope");
  });

  it("given the join step and no network, then it stays on join", () => {
    assert.equal(validStep(ctx({ hasNet: false }), "join", null), "join");
  });
});

describe("nextStep and prevStep", () => {
  it("given the os step on a six-step path, then next is browser", () => {
    assert.equal(nextStep(ctx(), "os"), "browser");
  });

  it("given the os step on a six-step path, then prev is scope", () => {
    assert.equal(prevStep(ctx(), "os"), "scope");
  });

  it("given the network and the scope step, then there is no previous step", () => {
    assert.equal(prevStep(ctx(), "scope"), null);
  });

  it("given no network and the scope step, then prev is join", () => {
    assert.equal(prevStep(ctx({ hasNet: false }), "scope"), "join");
  });

  it("given the exit step, then there is no next step", () => {
    assert.equal(nextStep(ctx(), "exit"), null);
  });

  it("given a step outside the path, then next and prev are null", () => {
    const c = ctx({ scope: "browser", os: "ios" });
    assert.equal(nextStep(c, "browser"), null);
    assert.equal(prevStep(c, "browser"), null);
  });
});

describe("stepProgress", () => {
  it("given the guide step on a six-step path, then it is step 5 of 6", () => {
    assert.deepEqual(stepProgress(ctx(), "guide"), { n: 5, m: 6 });
  });

  it("given the join step, then it counts as step 1", () => {
    assert.deepEqual(stepProgress(ctx({ hasNet: false }), "join"), { n: 1, m: 6 });
  });

  it("given a step outside the path, then n is 1 and m is the path length", () => {
    assert.deepEqual(stepProgress(ctx({ proxyOn: false }), "guide"), { n: 1, m: 3 });
  });
});

describe("navKind", () => {
  it("given the join step, then the screen has its own buttons", () => {
    assert.equal(navKind(ctx({ hasNet: false }), "join"), null);
  });

  it("given the scope step with the proxy on and no answer, then the button is skip", () => {
    assert.equal(navKind(ctx({ scope: null }), "scope"), "skip");
  });

  it("given the scope step with an answer, then there is no forward button", () => {
    assert.equal(navKind(ctx({ scope: "system" }), "scope"), null);
  });

  it("given the os or browser question, then there is no forward button", () => {
    assert.deepEqual([navKind(ctx(), "os"), navKind(ctx(), "browser")], [null, null]);
  });

  it("given the instruction, then the button is next", () => {
    assert.equal(navKind(ctx(), "guide"), "next");
  });

  it("given the scope step with the proxy off, then the button is next", () => {
    assert.equal(navKind(ctx({ proxyOn: false, scope: null }), "scope"), "next");
  });

  it("given the exit step on a computer, then the button is finish", () => {
    assert.equal(navKind(ctx(), "exit"), "finish");
  });

  it("given the exit step in the app, then the button is next", () => {
    assert.equal(navKind(ctx({ inApp: true }), "exit"), "next");
  });

  it("given the connect step, then the button is done", () => {
    assert.equal(navKind(ctx({ inApp: true }), "connect"), "done");
  });
});

describe("markOnEnter", () => {
  it("given the exit step, then the mark is exit", () => {
    assert.equal(markOnEnter("exit"), "exit");
  });

  it("given the guide step, then there is no mark", () => {
    assert.equal(markOnEnter("guide"), null);
  });
});

describe("isPhoneOs", () => {
  it("given android or ios, then it is a phone", () => {
    assert.equal(isPhoneOs("android"), true);
    assert.equal(isPhoneOs("ios"), true);
  });

  it("given mac, then it is not a phone", () => {
    assert.equal(isPhoneOs("mac"), false);
  });
});

describe("joinedLineOn", () => {
  it("given the network and the second step of the path, then the joined line is shown", () => {
    assert.equal(joinedLineOn(ctx(), "scope"), true);
  });

  it("given the network and a later row of the questions page, then the joined line is still shown", () => {
    assert.equal(joinedLineOn(ctx(), "os"), true);
  });

  it("given the network and the exit screen, then the joined line is not shown", () => {
    assert.equal(joinedLineOn(ctx(), "exit"), false);
  });

  it("given no network, then the joined line is not shown", () => {
    assert.equal(joinedLineOn(ctx({ hasNet: false }), "scope"), false);
  });
});

describe("stripSteps", () => {
  const states = (c, cur, seen = []) => stripSteps(c, cur, new Set(seen)).map((t) => `${t.id}:${t.state}`);

  it("given the os step on macOS, then joined and scope are done, the rest locked", () => {
    assert.deepEqual(states(ctx(), "os"), [
      "join:done", "scope:done", "os:current", "browser:locked", "guide:locked", "exit:locked",
    ]);
  });

  it("given a step right of the current one was seen, then it is open", () => {
    assert.deepEqual(states(ctx(), "scope", ["scope", "os", "browser"]), [
      "join:done", "scope:current", "os:open", "browser:open", "guide:locked", "exit:locked",
    ]);
  });

  it("given the scope switched to one browser, then the os step leaves the strip", () => {
    const ids = stripSteps(ctx({ scope: "browser" }), "scope", new Set(["os"])).map((t) => t.id);
    assert.deepEqual(ids, ["join", "scope", "browser", "guide", "exit"]);
  });

  it("given the question was skipped, then the strip is join, scope, exit", () => {
    assert.deepEqual(states(ctx({ scope: null, skipped: true }), "exit", ["exit"]), ["join:done", "scope:done", "exit:current"]);
  });

  it("given the app page, then the strip is join, exit, connect", () => {
    assert.deepEqual(states(ctx({ inApp: true }), "exit"), ["join:done", "exit:current", "connect:locked"]);
  });

  it("given no network on the join step, then join is current", () => {
    assert.equal(states(ctx({ hasNet: false }), "join")[0], "join:current");
  });
});

describe("screenOf", () => {
  it("given the questions and the instruction, then they share the choices screen", () => {
    assert.deepEqual(["scope", "os", "browser", "guide"].map(screenOf), ["choices", "choices", "choices", "choices"]);
  });

  it("given join, exit and connect, then each has its own screen", () => {
    assert.deepEqual(["join", "exit", "connect"].map(screenOf), ["join", "exit", "connect"]);
  });
});

describe("visibleRows", () => {
  it("given the cursor on the first question, then only that row is shown", () => {
    assert.deepEqual(visibleRows(ctx(), "scope"), ["scope"]);
  });

  it("given the cursor on the browser question, then the rows down to it are shown", () => {
    assert.deepEqual(visibleRows(ctx(), "browser"), ["scope", "os", "browser"]);
  });

  it("given the cursor on the instruction, then every row is shown", () => {
    assert.deepEqual(visibleRows(ctx(), "guide"), ["scope", "os", "browser", "guide"]);
  });

  it("given the browser scope, then the system row is not among the rows", () => {
    assert.deepEqual(visibleRows(ctx({ scope: "browser" }), "guide"), ["scope", "browser", "guide"]);
  });

  it("given the exit screen, then all the rows of the path are returned", () => {
    assert.deepEqual(visibleRows(ctx(), "exit"), ["scope", "os", "browser", "guide"]);
  });

  it("given the app page, then there are no rows", () => {
    assert.deepEqual(visibleRows(ctx({ inApp: true }), "exit"), []);
  });
});

describe("reanchor", () => {
  it("given the cursor step left the path, then the next step still on the path is taken", () => {
    const before = wizardPath(ctx());
    assert.equal(reanchor(before, wizardPath(ctx({ scope: "browser" })), "os"), "browser");
  });

  it("given the cursor step stays on the path, then it is kept", () => {
    const before = wizardPath(ctx());
    assert.equal(reanchor(before, wizardPath(ctx({ scope: "browser" })), "guide"), "guide");
  });
});

describe("settleStep", () => {
  it("given no scope answer, then the cursor stays on scope", () => {
    assert.equal(settleStep(ctx({ scope: null }), "scope"), "scope");
  });

  it("given an answered scope, then the cursor passes os and browser to the instruction", () => {
    assert.equal(settleStep(ctx(), "scope"), "guide");
  });

  it("given the proxy off, then the cursor stays on scope", () => {
    assert.equal(settleStep(ctx({ proxyOn: false }), "scope"), "scope");
  });
});
