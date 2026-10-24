/* What a pasted invite is, before it is used (`inspect-preview.ts`): the request `POST /api/network/inspect`, the one-line text
 * and the composable that asks after a pause in typing and drops answers to strings that are no longer in the field. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { nextTick, ref } from "vue";
import { INSPECT_DELAY_MS, inspectKey, previewText, useInspectPreview } from "./inspect-preview.ts";

const KEY = "mesh-invite2-alpha-beta-gamma";
const OTHER = "mesh-invite2-delta-epsilon-zeta";
const MANAGED_WORK = { label: "work", kind: "Managed", id_hex: "ab12" };
const PEER_HOME = { label: "home", kind: "Peer", id_hex: "cd34" };
const MANAGED_LINE = 'Network "work" · Managed: admins decide who joins';
const PEER_LINE = 'Network "home" · Peer: every member can invite';

describe("inspectKey", () => {
  it("posts the key to the inspect address and returns the node's answer", async () => {
    const w = setup({ answers: [MANAGED_WORK] });
    assert.deepEqual(await inspectKey(w.api, KEY), MANAGED_WORK);
    assert.deepEqual(w.requests, [{ method: "POST", path: "/api/network/inspect", body: { key: KEY } }]);
  });

  it("an answer without a name comes back with a null label", async () => {
    const w = setup({ answers: [{ label: null, kind: "Peer", id_hex: "ef56" }] });
    assert.equal((await inspectKey(w.api, KEY)).label, null);
  });

  describe("a refusal is not an error", () => {
    it("422 — null", async () => {
      const w = setup({ answers: [refused(422, "invalid_key")] });
      assert.equal(await inspectKey(w.api, KEY), null);
    });

    it("a node that is down — null", async () => {
      const w = setup({ answers: [new TypeError("Failed to fetch")] });
      assert.equal(await inspectKey(w.api, KEY), null);
    });

    it("an older node without the address (404) — null", async () => {
      const w = setup({ answers: [refused(404, "not_found")] });
      assert.equal(await inspectKey(w.api, KEY), null);
    });
  });
});

describe("previewText", () => {
  describe("a named network", () => {
    it("Managed — admins decide who joins", () => assert.equal(previewText(MANAGED_WORK), MANAGED_LINE));
    it("Peer — every member can invite", () => assert.equal(previewText(PEER_HOME), PEER_LINE));
  });

  describe("a network without a name", () => {
    it("Managed — no empty quotes", () => {
      assert.equal(previewText({ ...MANAGED_WORK, label: null }), "Network · Managed: admins decide who joins");
    });

    it("Peer — no empty quotes", () => {
      assert.equal(previewText({ ...PEER_HOME, label: null }), "Network · Peer: every member can invite");
    });
  });
});

describe("useInspectPreview", () => {
  describe("typing", () => {
    it("asks nothing and shows nothing until the pause is over", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed(KEY);
      assert.deepEqual(w.requests, []);
      assert.equal(w.preview.value, "");
      assert.equal(w.timers.pending().length, 1, "one timer is waiting");
      assert.ok(w.timers.pending()[0].ms > 0, "the pause is not zero");
    });

    it("after the pause shows the text of the answer", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed(KEY);
      await w.pauseOver();
      assert.equal(w.preview.value, MANAGED_LINE);
      assert.deepEqual(w.requests.map((r) => r.body), [{ key: KEY }]);
    });

    it("a Peer network gets its own line", async () => {
      const w = await setup({ answers: [PEER_HOME] }).typed(KEY);
      await w.pauseOver();
      assert.equal(w.preview.value, PEER_LINE);
    });

    it("keys typed in quick succession cost one request, for the last one", async () => {
      const w = setup({ answers: [MANAGED_WORK] });
      await w.typed("mesh-inv");
      await w.typed("mesh-invite2-alp");
      await w.typed(KEY);
      assert.equal(w.timers.pending().length, 1, "earlier timers are cancelled");
      await w.pauseOver();
      assert.deepEqual(w.requests.map((r) => r.body), [{ key: KEY }]);
    });

    it("the pause constant is the one the timer is set with", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed(KEY);
      assert.equal(w.timers.pending()[0].ms, INSPECT_DELAY_MS);
    });
  });

  describe("an empty field", () => {
    it("never asks", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed("");
      await w.pauseOver();
      assert.deepEqual(w.requests, []);
      assert.equal(w.preview.value, "");
    });

    it("spaces only — never asks", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed("   ");
      await w.pauseOver();
      assert.deepEqual(w.requests, []);
    });

    it("emptying the field removes the line at once", async () => {
      const w = await setup({ answers: [MANAGED_WORK] }).typed(KEY);
      await w.pauseOver();
      await w.typed("");
      assert.equal(w.preview.value, "");
    });
  });

  describe("the field changes after the line is shown", () => {
    it("the old line goes at once: it describes another string", async () => {
      const w = await setup({ answers: [MANAGED_WORK, PEER_HOME] }).typed(KEY);
      await w.pauseOver();
      await w.typed(OTHER);
      assert.equal(w.preview.value, "");
    });

    it("the new string gets its own line after its pause", async () => {
      const w = await setup({ answers: [MANAGED_WORK, PEER_HOME] }).typed(KEY);
      await w.pauseOver();
      await w.typed(OTHER);
      await w.pauseOver();
      assert.equal(w.preview.value, PEER_LINE);
    });
  });

  describe("the answer to an old string arrives late", () => {
    it("after the field was changed — it is not shown", async () => {
      const w = await setup({ deferred: true }).typed(KEY);
      await w.pauseOver();
      await w.typed(OTHER);
      await w.answer(0, MANAGED_WORK);
      assert.equal(w.preview.value, "");
    });

    it("after the newer string was answered — it does not replace the newer line", async () => {
      const w = await setup({ deferred: true }).typed(KEY);
      await w.pauseOver();
      await w.typed(OTHER);
      await w.pauseOver();
      await w.answer(1, PEER_HOME);
      await w.answer(0, MANAGED_WORK);
      assert.equal(w.preview.value, PEER_LINE);
    });

    it("after the field was emptied — nothing is shown", async () => {
      const w = await setup({ deferred: true }).typed(KEY);
      await w.pauseOver();
      await w.typed("");
      await w.answer(0, MANAGED_WORK);
      assert.equal(w.preview.value, "");
    });
  });

  describe("a refusal", () => {
    it("422 — no line and no error", async () => {
      const w = await setup({ answers: [refused(422, "invalid_key")] }).typed(KEY);
      await w.pauseOver();
      assert.equal(w.preview.value, "");
    });

    it("after a line was shown, a refused string leaves no line of the earlier one", async () => {
      const w = await setup({ answers: [MANAGED_WORK, refused(422, "invalid_key")] }).typed(KEY);
      await w.pauseOver();
      await w.typed(OTHER);
      await w.pauseOver();
      assert.equal(w.preview.value, "");
    });

    it("a node that is down — no line, and a later string is still asked", async () => {
      const w = await setup({ answers: [new TypeError("Failed to fetch"), MANAGED_WORK] }).typed(KEY);
      await w.pauseOver();
      assert.equal(w.preview.value, "");
      await w.typed(OTHER);
      await w.pauseOver();
      assert.equal(w.preview.value, MANAGED_LINE);
    });
  });
});

const refused = (status, code) => Object.assign(new Error(code), { status, code });

// Preconditions: the answers of the node in the order of requests (an `Error` is thrown; with `deferred` every request waits for
// `answer(i, value)`), and a timer the test fires by hand. The field is a ref the test types into.
function setup({ answers = [], deferred = false } = {}) {
  const requests = [];
  const waiting = [];
  const api = vi.fn((method, path, body) => {
    requests.push({ method, path, body: structuredClone(body) });
    if (deferred) return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
    const answer = answers[requests.length - 1];
    return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer);
  });
  const timers = fakeTimers();
  const key = ref("");
  const { preview } = useInspectPreview(() => key.value, api, timers);
  const w = {
    requests, preview, timers, api,
    async typed(value) {
      key.value = value;
      await nextTick();
      return w;
    },
    async pauseOver() {
      timers.fireAll();
      await flush();
    },
    async answer(index, value) {
      if (value instanceof Error) waiting[index].reject(value);
      else waiting[index].resolve(value);
      await flush();
    },
  };
  return w;
}

function fakeTimers() {
  let nextId = 1;
  const queue = new Map();
  return {
    setTimer(fn, ms) {
      const id = nextId++;
      queue.set(id, { fn, ms });
      return id;
    },
    clearTimer: (id) => queue.delete(id),
    pending: () => [...queue.values()],
    fireAll() {
      const due = [...queue.values()];
      queue.clear();
      due.forEach((t) => t.fn());
    },
  };
}

const flush = async () => {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
  await nextTick();
};
