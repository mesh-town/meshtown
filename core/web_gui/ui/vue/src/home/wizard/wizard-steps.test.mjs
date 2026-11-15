/* The step graph of the setup wizard (`wizard-steps.ts`): the path by the answers, the start step, the
 * neighbours, the progress counter, the kind of the forward button, the setup mark and the joined line. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  isPhoneOs, joinedLineOn, markOnEnter, navKind, nextStep, prevStep, reanchor, screenOf, settleStep, startStep, stepProgress, stripSteps, validStep, visibleRows, wizardPath,
} from "./wizard-steps.ts";

const ctx = (over = {}) => ({ hasNet: true, inApp: false, proxyOn: true, scope: "system", os: "mac", skipped: false, router: false, ...over });

describe("wizardPath", () => {
  it("exit right after join on desktop", () => {
    for (const c of [ctx(), ctx({ scope: "browser" }), ctx({ proxyOn: false }), ctx({ skipped: true, scope: null }), ctx({ scope: "browser", os: "ios" })]) {
      assert.deepEqual(wizardPath(c).slice(0, 2), ["join", "exit"]);
    }
  });

  it("router path puts exit second", () => {
    assert.deepEqual(wizardPath(ctx({ router: true })), ["join", "exit", "os", "guide"]);
  });

  it("app path unchanged", () => {
    assert.deepEqual(wizardPath(ctx({ inApp: true, router: true })), ["join", "exit", "connect"]);
  });

  describe("in the app", () => {
    it("given the app page, then the path is join, exit, connect", () => {
      assert.deepEqual(wizardPath(ctx({ inApp: true })), ["join", "exit", "connect"]);
    });
  });

  describe("proxy is off", () => {
    it("given the proxy is off, then the path is join, exit, scope", () => {
      assert.deepEqual(wizardPath(ctx({ proxyOn: false })), ["join", "exit", "scope"]);
    });
  });

  describe("the scope question was skipped", () => {
    it("given skipped, then the path is join, exit, scope", () => {
      assert.deepEqual(wizardPath(ctx({ scope: null, skipped: true })), ["join", "exit", "scope"]);
    });
  });

  describe("scope is the whole system", () => {
    it("given system on macOS, then the path has os and guide, no browser", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "system", os: "mac" })), ["join", "exit", "scope", "os", "guide"]);
    });

    it("given no answer yet on macOS, then the path is the recommended one without browser", () => {
      assert.deepEqual(wizardPath(ctx({ scope: null, os: "mac" })), ["join", "exit", "scope", "os", "guide"]);
    });

    it("given system on any computer, then the path has no browser step", () => {
      for (const os of ["mac", "windows", "linux"]) {
        assert.ok(!wizardPath(ctx({ scope: "system", os })).includes("browser"), os);
      }
    });

    it("given system on Android, then there is no browser step", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "system", os: "android" })), ["join", "exit", "scope", "os", "guide"]);
    });
  });

  describe("scope is one browser", () => {
    it("given browser on macOS, then the path is exit, scope, browser, guide", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "browser", os: "mac" })), ["join", "exit", "scope", "browser", "guide"]);
    });

    it("given browser on Linux, then there is no os step", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "browser", os: "linux" })), ["join", "exit", "scope", "browser", "guide"]);
    });

    it("given browser on iOS, then there are neither os nor browser steps", () => {
      assert.deepEqual(wizardPath(ctx({ scope: "browser", os: "ios" })), ["join", "exit", "scope", "guide"]);
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

  it("given no mark, then the start is exit", () => {
    assert.equal(startStep(ctx({ scope: null }), null), "exit");
  });

  it("given an unknown mark, then it is treated as no mark and the start is exit", () => {
    assert.equal(startStep(ctx({ scope: null }), "garbage"), "exit");
  });

  it("given the browser mark and a remembered scope, then the answered rows are passed and the start is the instruction", () => {
    assert.equal(startStep(ctx(), "browser"), "guide");
  });

  it("given the browser mark and the proxy off, then the start is scope even with a remembered answer", () => {
    assert.equal(startStep(ctx({ proxyOn: false }), "browser"), "scope");
  });

  it("given the exit mark and a remembered scope, then the start is still exit", () => {
    assert.equal(startStep(ctx(), "exit"), "exit");
  });

  it("given the browser mark on a router, then the start is the system step", () => {
    assert.equal(startStep(ctx({ router: true }), "browser"), "guide");
  });
});

describe("validStep", () => {
  it("given the current step is on the path, then it stays", () => {
    assert.equal(validStep(ctx(), "guide", null), "guide");
  });

  it("given the join step after the network appeared, then the wizard moves to the start step", () => {
    assert.equal(validStep(ctx({ scope: null }), "join", "browser"), "scope");
  });

  it("given the join step and the exit mark after the network appeared, then it moves to exit", () => {
    assert.equal(validStep(ctx(), "join", "exit"), "exit");
  });

  it("given the browser step after the proxy was turned off, then it falls back to scope", () => {
    assert.equal(validStep(ctx({ proxyOn: false }), "browser", "browser"), "scope");
  });

  it("given the join step and no network, then it stays on join", () => {
    assert.equal(validStep(ctx({ hasNet: false }), "join", null), "join");
  });

  it("given the instruction and no scope answer, then the cursor goes to the start step", () => {
    assert.equal(validStep(ctx({ scope: null }), "guide", "browser"), "scope");
  });
});

describe("nextStep and prevStep", () => {
  it("given the os step on the all-apps path, then next is the instruction", () => {
    assert.equal(nextStep(ctx(), "os"), "guide");
  });

  it("given the scope step of the browser path, then next is browser", () => {
    assert.equal(nextStep(ctx({ scope: "browser" }), "scope"), "browser");
  });

  it("given the os step on the all-apps path, then prev is scope", () => {
    assert.equal(prevStep(ctx(), "os"), "scope");
  });

  it("given the network and the exit step, then there is no previous step", () => {
    assert.equal(prevStep(ctx(), "exit"), null);
  });

  it("given the network and the scope step, then prev is exit", () => {
    assert.equal(prevStep(ctx(), "scope"), "exit");
  });

  it("given no network and the exit step, then prev is join", () => {
    assert.equal(prevStep(ctx({ hasNet: false }), "exit"), "join");
  });

  it("given the instruction, then there is no next step", () => {
    assert.equal(nextStep(ctx(), "guide"), null);
  });

  it("given the exit step on a computer, then next is scope", () => {
    assert.equal(nextStep(ctx(), "exit"), "scope");
  });

  it("given a step outside the path, then next and prev are null", () => {
    const c = ctx({ scope: "browser", os: "ios" });
    assert.equal(nextStep(c, "browser"), null);
    assert.equal(prevStep(c, "browser"), null);
  });
});

describe("stepProgress", () => {
  it("given the guide step on the all-apps path, then it is step 5 of 5", () => {
    assert.deepEqual(stepProgress(ctx(), "guide"), { n: 5, m: 5 });
  });

  it("given the join step, then it counts as step 1", () => {
    assert.deepEqual(stepProgress(ctx({ hasNet: false }), "join"), { n: 1, m: 5 });
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

  it("given the instruction, then the button is finish", () => {
    assert.equal(navKind(ctx(), "guide"), "finish");
  });

  it("given the scope step with the proxy off, then it is the last step and the button is finish", () => {
    assert.equal(navKind(ctx({ proxyOn: false, scope: null }), "scope"), "finish");
  });

  it("exit is next not finish on desktop", () => {
    assert.equal(navKind(ctx(), "exit"), "next");
    assert.equal(navKind(ctx({ proxyOn: false }), "exit"), "next");
    assert.equal(navKind(ctx({ router: true }), "exit"), "next");
  });

  it("finish is on the last step of the path", () => {
    const paths = [ctx(), ctx({ scope: "browser" }), ctx({ proxyOn: false }), ctx({ router: true }), ctx({ skipped: true, scope: null })];
    for (const c of paths) {
      const path = wizardPath(c);
      const finishing = path.filter((id) => navKind(c, id) === "finish");
      assert.deepEqual(finishing, [path[path.length - 1]], path.join());
    }
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

  it("given a step after exit, then the mark is browser", () => {
    assert.deepEqual(["scope", "os", "browser", "guide"].map(markOnEnter), ["browser", "browser", "browser", "browser"]);
  });

  it("given join or connect, then there is no mark", () => {
    assert.deepEqual([markOnEnter("join"), markOnEnter("connect")], [null, null]);
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
    assert.equal(joinedLineOn(ctx(), "exit"), true);
  });

  it("given the network and the questions page, then the joined line is not shown", () => {
    assert.equal(joinedLineOn(ctx(), "scope"), false);
    assert.equal(joinedLineOn(ctx(), "os"), false);
  });

  it("given no network, then the joined line is not shown", () => {
    assert.equal(joinedLineOn(ctx({ hasNet: false }), "exit"), false);
  });
});

describe("stripSteps", () => {
  const states = (c, cur, seen = []) => stripSteps(c, cur, new Set(seen)).map((t) => `${t.id}:${t.state}`);

  it("given the os step on macOS, then joined and scope are done, the rest locked", () => {
    assert.deepEqual(states(ctx(), "os"), [
      "join:done", "exit:done", "scope:done", "os:current", "guide:locked",
    ]);
  });

  it("given a step right of the current one was seen, then it is open", () => {
    assert.deepEqual(states(ctx({ scope: "browser" }), "scope", ["scope", "browser"]), [
      "join:done", "exit:done", "scope:current", "browser:open", "guide:locked",
    ]);
  });

  it("given the scope switched to one browser, then the os step leaves the strip", () => {
    const ids = stripSteps(ctx({ scope: "browser" }), "scope", new Set(["os"])).map((t) => t.id);
    assert.deepEqual(ids, ["join", "exit", "scope", "browser", "guide"]);
  });

  it("given the question was skipped, then the strip is join, exit, scope", () => {
    assert.deepEqual(states(ctx({ scope: null, skipped: true }), "scope", ["scope"]), ["join:done", "exit:done", "scope:current"]);
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
    assert.deepEqual(visibleRows(ctx({ scope: "browser" }), "browser"), ["scope", "browser"]);
  });

  it("given the cursor on the instruction, then every row is shown", () => {
    assert.deepEqual(visibleRows(ctx(), "guide"), ["scope", "os", "guide"]);
  });

  it("given the browser scope, then the system row is not among the rows", () => {
    assert.deepEqual(visibleRows(ctx({ scope: "browser" }), "guide"), ["scope", "browser", "guide"]);
  });

  it("given the exit screen, then all the rows of the path are returned", () => {
    assert.deepEqual(visibleRows(ctx(), "exit"), ["scope", "os", "guide"]);
  });

  it("given the app page, then there are no rows", () => {
    assert.deepEqual(visibleRows(ctx({ inApp: true }), "exit"), []);
  });
});

describe("reanchor", () => {
  it("given the cursor step left the path, then the next step still on the path is taken", () => {
    const before = wizardPath(ctx());
    assert.equal(reanchor(before, wizardPath(ctx({ scope: "browser" })), "os"), "guide");
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

  it("given an answered scope, then the cursor passes os to the instruction", () => {
    assert.equal(settleStep(ctx(), "scope"), "guide");
  });

  it("given the proxy off, then the cursor stays on scope", () => {
    assert.equal(settleStep(ctx({ proxyOn: false }), "scope"), "scope");
  });
});

describe("on a router", () => {
  it("given the proxy on, then the path is join, exit, os, guide whatever the scope", () => {
    for (const scope of [null, "system", "browser"]) {
      assert.deepEqual(wizardPath(ctx({ router: true, scope })), ["join", "exit", "os", "guide"], String(scope));
    }
  });

  it("given no scope and the cursor on guide, then the cursor stays", () => {
    assert.equal(validStep(ctx({ router: true, scope: null }), "guide", null), "guide");
  });

  it("given the proxy off, then the path is join, exit, scope", () => {
    assert.deepEqual(wizardPath(ctx({ router: true, proxyOn: false })), ["join", "exit", "scope"]);
  });

  it("given no router, no scope and the cursor on guide, then the cursor goes back to the start", () => {
    assert.notEqual(validStep(ctx({ scope: null }), "guide", "browser"), "guide");
  });
});
