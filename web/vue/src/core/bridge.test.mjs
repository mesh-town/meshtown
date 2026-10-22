/* `bridge.ts`: section signals → the shell (`meshtown:changed`); `mesh:render` and `connectBridge` are removed. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as bridge from "./bridge.ts";
import { notifyChanged, EV_CHANGED } from "./bridge.ts";

describe("removed", () => {
  it("connectBridge and EV_RENDER are not exported from the module", () => {
    assert.equal("connectBridge" in bridge, false);
    assert.equal("EV_RENDER" in bridge, false);
  });
});

describe("notifyChanged", () => {
  describe("allowed what", () => {
    for (const what of ["config", "state", "token", "render", "stopped", "stopping"]) {
      it(`${what} — one meshtown:changed event with detail {what}`, () => {
        const { target, seen } = setup();
        notifyChanged(target, what);
        assert.deepEqual(seen, [{ what }]);
      });
    }
  });

  describe("invalid what", () => {
    it("throws TypeError, no event went out", () => {
      const { target, seen } = setup();
      assert.throws(() => notifyChanged(target, "x"), TypeError);
      assert.deepEqual(seen, []);
    });
  });
});

function setup() {
  const target = new EventTarget();
  const seen = [];
  target.addEventListener(EV_CHANGED, (ev) => seen.push(ev.detail));
  return { target, seen };
}
