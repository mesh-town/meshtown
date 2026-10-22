/* `conn.ts`: the page's connection state with the node — poll backoff, strips, error classification. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { NET_NAME } from "../common/words.ts";
import * as connModule from "./conn.ts";
import {
  BACKOFF_STEPS, POLL_VISIBLE_MS, classify, createConn, nextBackoff, onBadOrigin, onBadToken,
  onFailure, onStopping, onSuccess, stripView,
} from "./conn.ts";

describe("nextBackoff", () => {
  it("no failures — the visible poll interval", () => {
    assert.equal(nextBackoff(0), 2000);
    assert.equal(nextBackoff(0), POLL_VISIBLE_MS);
  });

  it("grows in steps with the number of consecutive failures", () => {
    assert.equal(nextBackoff(1), BACKOFF_STEPS[1]);
    assert.equal(nextBackoff(2), 5000);
    assert.equal(nextBackoff(3), 15000);
  });

  it("does not grow beyond the last step", () => {
    assert.equal(nextBackoff(99), BACKOFF_STEPS[BACKOFF_STEPS.length - 1]);
    assert.equal(nextBackoff(99), 15000);
  });
});

describe("onFailure", () => {
  it("one failure — no strip, counter 1", () => {
    const conn = createConn();
    onFailure(conn);
    assert.equal(conn.failures, 1);
    assert.equal(conn.strip, null);
  });

  it("two failures in a row — \"Mesh Town is not responding\", the backoff grew", () => {
    const conn = createConn();
    onFailure(conn);
    onFailure(conn);
    assert.deepEqual(conn.strip, { kind: "disconnected", text: "Mesh Town is not responding" });
    assert.equal(conn.backoffMs, 5000);
  });

  it("with a stopped node no connection strip appears", () => {
    const conn = createConn();
    conn.stopped = true;
    onFailure(conn);
    onFailure(conn);
    assert.equal(conn.strip, null);
  });

  it("while waiting for a token no connection strip appears", () => {
    const conn = createConn();
    conn.awaitingToken = true;
    onFailure(conn);
    onFailure(conn);
    assert.equal(conn.strip, null);
  });
});

describe("onSuccess", () => {
  it("after \"No connection\" — \"restarted\" and the text \"Mesh Town is responding again\"", () => {
    const conn = createConn();
    onFailure(conn);
    onFailure(conn);
    assert.equal(onSuccess(conn), "restarted");
    assert.deepEqual(conn.strip, { kind: "restarted", text: NET_NAME + " is responding again" });
    assert.equal(conn.failures, 0);
    assert.equal(conn.backoffMs, POLL_VISIBLE_MS);
  });

  it("after normal operation — null, no strip", () => {
    const conn = createConn();
    assert.equal(onSuccess(conn), null);
    assert.equal(conn.strip, null);
  });

  it("one past failure without a strip — not \"restarted\"", () => {
    const conn = createConn();
    onFailure(conn);
    assert.equal(onSuccess(conn), null);
    assert.equal(conn.strip, null);
  });

  it("clears the stop flag", () => {
    const conn = createConn();
    onStopping(conn);
    onSuccess(conn);
    assert.equal(conn.stopped, false);
  });
});

describe("onSuccess after stopped", () => {
  it("the strip is cleared (strip === null), stopped is cleared, returns null", () => {
    // given
    const conn = createConn();
    onStopping(conn);
    // when
    const result = onSuccess(conn);
    // then
    assert.equal(conn.strip, null);
    assert.equal(conn.stopped, false);
    assert.equal(result, null);
  });

  it("does not clear someone else's strip (token)", () => {
    const conn = createConn();
    onBadToken(conn);
    onSuccess(conn);
    assert.equal(conn.strip.kind, "bad_token");
  });

  it("the POLL_STOPPED_MS export — 15000", () => {
    assert.equal(connModule.POLL_STOPPED_MS, 15000);
  });
});

describe("strips by response code", () => {
  it("onBadToken — wait for the token, text about the tray", () => {
    const conn = createConn();
    onBadToken(conn);
    assert.equal(conn.awaitingToken, true);
    assert.deepEqual(conn.strip, { kind: "bad_token", text: "The page was opened from an old or incomplete link — open it again: the Mesh Town icon near the clock → \"Open Mesh Town…\". Or paste the new link:" });
  });

  it("onBadOrigin with message — the text is single, message is not shown", () => {
    const conn = createConn();
    onBadOrigin(conn, { message: "foreign address" });
    assert.deepEqual(conn.strip, { kind: "bad_origin", text: "The page was opened from the wrong address — open it from the Mesh Town icon near the clock" });
  });

  it("onBadOrigin without message or body — the same text", () => {
    const a = createConn();
    onBadOrigin(a, {});
    const b = createConn();
    onBadOrigin(b, null);
    assert.equal(a.strip.text, "The page was opened from the wrong address — open it from the Mesh Town icon near the clock");
    assert.equal(b.strip.text, "The page was opened from the wrong address — open it from the Mesh Town icon near the clock");
  });

  it("onBadOrigin writes to the console only on first entry and only the node's message", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const conn = createConn();
    onBadOrigin(conn, { message: "foreign address" });
    onBadOrigin(conn, { message: "foreign address" });
    assert.equal(warn.mock.calls.length, 1);
    assert.deepEqual(warn.mock.calls[0], ["Mesh Town:", "foreign address"]);
    onBadOrigin(createConn(), null);
    assert.equal(warn.mock.calls.length, 1);
  });

  it("onStopping — the program is stopped", () => {
    const conn = createConn();
    onStopping(conn);
    assert.equal(conn.stopped, true);
    assert.deepEqual(conn.strip, { kind: "stopped", text: NET_NAME + " is stopped — start it: the " + NET_NAME + " icon near the clock → \"Start " + NET_NAME + "\". You don't need to refresh the page." });
  });
});

describe("classify", () => {
  const cases = [
    [0, "failure"], [401, "token"], [403, "origin"], [503, "stopping"], [422, "alive"], [500, "alive"],
  ];
  for (const [status, want] of cases) {
    it(`status ${status} → ${want}`, () => {
      assert.equal(classify(new ApiError(status, "x")), want);
    });
  }

  it("not an ApiError (a failure without a status) — failure", () => {
    assert.equal(classify(new Error("boom")), "failure");
  });
});

describe("stripView", () => {
  it("no strip and no config changes — null", () => {
    assert.equal(stripView(createConn()), null);
  });

  it("configChanged without a connection strip — kind changed", () => {
    const conn = createConn();
    conn.configChanged = true;
    const view = stripView(conn);
    assert.equal(view.kind, "changed");
    assert.match(view.text, /Settings were changed in another tab or by Mesh Town itself/);
  });

  it("the connection strip outranks configChanged", () => {
    const conn = createConn();
    conn.configChanged = true;
    onStopping(conn);
    assert.equal(stripView(conn).kind, "stopped");
  });
});
