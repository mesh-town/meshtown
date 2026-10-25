/* The pure logic of the Log section (`log-view.ts`): `formatLogTime` and filter cases, merging a poll,
 * the ring, texts. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { NET_A, NET_B } from "../testing/overview-fixtures.mjs";
import {
  GAP_TEXT, chipTags, downloadUrl, effectiveLogNet, emptyFeed, entryPasses, formatLogTime, lineNetTag, logNetFilters, mergePoll,
  pollErrorText, pollPath, ringHint, logRingMaxLines, trimRing, visibleEntries,
} from "./log-view.ts";

const line = (seq, tag = "net", text = "line " + seq) => ({ seq, t: seq, tag, text });

describe("formatLogTime", () => {
  it("pads a short number with spaces on the left to 7 characters", () => assert.equal(formatLogTime(0), "+    0.0s"));
  it("a number longer than the field is left as is", () => assert.equal(formatLogTime(1234567.8), "+1234567.8s"));
});

describe("entryPasses", () => {
  it("a gap entry passes any filter", () => {
    assert.equal(entryPasses({ gap: true }, "net", "no match"), true);
  });
  it("the entry tag matches the active one — passes, otherwise not", () => {
    assert.equal(entryPasses(line(1, "net"), "net", ""), true);
    assert.equal(entryPasses(line(1, "err"), "net", ""), false);
  });
  it("search is case-insensitive", () => {
    assert.equal(entryPasses({ tag: "info", text: "warning: retry" }, "all", "Warn"), true);
    assert.equal(entryPasses({ tag: "info", text: "all is fine" }, "all", "warn"), false);
  });
  it("tag and search both set — both must match", () => {
    assert.equal(entryPasses({ tag: "net", text: "warning here" }, "net", "warn"), true);
    assert.equal(entryPasses({ tag: "net", text: "all is fine" }, "net", "warn"), false);
  });
  it("an empty filter passes everything", () => assert.equal(entryPasses({ tag: "x", text: "y" }, "all", ""), true));
});

describe("visibleEntries", () => {
  it("keeps the gap and matching lines, preserves the order", () => {
    const lines = [line(1, "net"), { gap: true, key: "gap" }, line(2, "err"), line(3, "net")];
    assert.deepEqual(visibleEntries(lines, "net", "").map((e) => e.seq), [1, undefined, 3]);
  });
});

describe("trimRing", () => {
  it("within the ring — unchanged", () => {
    const lines = [line(1), line(2)];
    assert.deepEqual(trimRing(lines, 2), lines);
  });
  it("beyond the ring — the oldest are lost", () => {
    assert.deepEqual(trimRing([line(1), line(2), line(3)], 2).map((e) => e.seq), [2, 3]);
  });
  it("the gap does not count toward the ring, but goes if it is the oldest", () => {
    const lines = [{ gap: true }, line(1), line(2)];
    assert.deepEqual(trimRing(lines, 2), lines);
    assert.deepEqual(trimRing(lines, 1).map((e) => e.seq), [2]);
  });
});

describe("mergePoll", () => {
  it("no response — the feed is the same", () => {
    const feed = emptyFeed();
    assert.equal(mergePoll(feed, null, 10), feed);
  });
  it("new lines are appended, lastSeq — by the last one, lines have a key", () => {
    const feed = mergePoll(emptyFeed(), { lines: [line(1), line(2)] }, 10);
    const next = mergePoll(feed, { lines: [line(3)] }, 10);
    assert.deepEqual(next.lines.map((e) => e.key), ["l1", "l2", "l3"]);
    assert.equal(next.lastSeq, 3);
  });
  it("an empty response with last_seq moves lastSeq, without it — no", () => {
    const feed = { lines: [line(1)], lastSeq: 1 };
    assert.equal(mergePoll(feed, { lines: [], last_seq: 7 }, 10).lastSeq, 7);
    assert.equal(mergePoll(feed, { lines: [] }, 10).lastSeq, 1);
  });
  it("gap — the feed starts anew with a gap, then the response lines", () => {
    const feed = { lines: [line(1)], lastSeq: 1 };
    const next = mergePoll(feed, { gap: true, lines: [line(9)] }, 10);
    assert.deepEqual(next.lines.map((e) => e.key), ["gap", "l9"]);
    assert.equal(next.lastSeq, 9);
  });
  it("ring: beyond max the old lines are dropped", () => {
    const feed = mergePoll(emptyFeed(), { lines: [line(1), line(2), line(3)] }, 2);
    assert.deepEqual(feed.lines.map((e) => e.seq), [2, 3]);
  });
  it("the source feed is not mutated", () => {
    const feed = { lines: [line(1)], lastSeq: 1 };
    mergePoll(feed, { lines: [line(2)] }, 10);
    assert.equal(feed.lines.length, 1);
  });
});

describe("texts and addresses", () => {
  it("ringHint with a log — the number of lines from the snapshot; without it — the fallback text \"2,000\"", () => {
    assert.match(ringHint({ ring_lines: 50 }), /the last 50 lines\. It starts over when Mesh Town restarts\. To keep it, click "Download \.log"\./);
    assert.match(ringHint(undefined), /the last 2,000 lines/);
    assert.match(ringHint({ ring_lines: 20000 }), /the last 20,000 lines/);
  });
  it("ringHint — the number of lines with a comma as the thousands separator", () => {
    assert.match(ringHint({ ring_lines: 12345 }), /12,345/);
  });
  it("chipTags and logRingMaxLines without the log block — fallback values", () => {
    assert.deepEqual(chipTags(undefined), []);
    assert.deepEqual(chipTags({ tags: ["net"] }), ["net"]);
    assert.equal(logRingMaxLines(undefined), 2000);
    assert.equal(logRingMaxLines({ ring_lines: 50 }), 50);
  });
  it("pollPath and downloadUrl", () => {
    assert.equal(pollPath(12), "/api/log?after=12&limit=500");
    assert.equal(downloadUrl("a b&c"), "/api/log/download?token=a%20b%26c");
  });
  it("pollErrorText: status 0 — silently, anything else — \"Couldn't update the log: …\"", () => {
    assert.equal(pollErrorText({ status: 0, message: "Node is not responding" }), "");
    assert.equal(pollErrorText({ status: 500, message: "Node responded with an error (code 500)" }), "Couldn't update the log: node responded with an error (code 500)");
    assert.equal(pollErrorText(new Error("")), "Couldn't update the log — try again");
  });
  it("pollErrorText: a response with a code from the page names the program, the name is written as is", () => {
    const err = new ApiError(500, "boom", API_ERROR_TEXT.http(500));
    assert.equal(pollErrorText(err), "Couldn't update the log: " + NET_NAME + " responded with an error (code 500)");
  });
  it("the gap text names the program, not the word \"node\"", () => {
    assert.equal(GAP_TEXT, "⋯ lines skipped here: " + NET_NAME + " erased them before the page could read them ⋯");
  });
});

describe("log by network", () => {
  describe("pollPath", () => {
    it("with a network filter adds &net=<slot> after the limit", () => {
      assert.equal(pollPath(5, NET_B), "/api/log?after=5&limit=500&net=k7q2xa");
    });
    it("without a filter the path is the same as before", () => {
      assert.equal(pollPath(5), "/api/log?after=5&limit=500");
      assert.equal(pollPath(5, null), "/api/log?after=5&limit=500");
    });
    it("the download address has no filter: the whole log", () => {
      assert.equal(downloadUrl("t"), "/api/log/download?token=t");
    });
  });

  describe("two networks", () => {
    it("filters are All networks, then every network that holds a key", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.deepEqual(logNetFilters(store), [
        { slot: null, label: "All networks" },
        { slot: NET_A, label: "friends" },
        { slot: NET_B, label: "my devices" },
      ]);
    });
    it("a slot without a key is not offered as a filter", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb"), net("m3n4pq", null, null)]);
      assert.deepEqual(logNetFilters(store).map((f) => f.slot), [null, NET_A, NET_B]);
    });
    it("the network that was left disappears from the filters", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb"), net("m3n4pq", "work", "cccc")]);
      leave(store, NET_B);
      assert.deepEqual(logNetFilters(store).map((f) => f.slot), [null, NET_A, "m3n4pq"]);
    });
    it("the selected network is kept while it is a filter", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.equal(effectiveLogNet(store, NET_B), NET_B);
      assert.equal(effectiveLogNet(store, null), null);
    });
    it("the selected network that was left falls back to All networks (null)", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb"), net("m3n4pq", "work", "cccc")]);
      leave(store, NET_B);
      assert.equal(effectiveLogNet(store, NET_B), null);
    });
    it("with one network left no selection survives", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      leave(store, NET_B);
      assert.equal(effectiveLogNet(store, NET_B), null);
      assert.equal(effectiveLogNet(store, NET_A), null);
    });
    it("a line of network B is tagged with its name in braces", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.equal(lineNetTag({ ...line(1), net: NET_B }, store), "{my devices}");
    });
    it("a line of the main network is tagged {friends}", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.equal(lineNetTag({ ...line(1), net: NET_A }, store), "{friends}");
    });
    it("a process line without net has no tag", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.equal(lineNetTag(line(1), store), "");
    });
    it("a line of an unknown slot is tagged with the slot id", () => {
      const store = setup([net(NET_A, "friends", "aaaa"), net(NET_B, "my devices", "bbbb")]);
      assert.equal(lineNetTag({ ...line(1), net: "zzzzzz" }, store), "{zzzzzz}");
    });
  });

  describe("one network", () => {
    it("there is no filter row", () => {
      assert.deepEqual(logNetFilters(setup([net(NET_A, "friends", "aaaa")])), []);
    });
    it("there is no filter row when the only network is not the main one", () => {
      assert.deepEqual(logNetFilters(setup([net(NET_A, null, null), net(NET_B, "my devices", "bbbb")])), []);
    });
    it("lines carry no tags, even when they have a net", () => {
      const store = setup([net(NET_A, "friends", "aaaa")]);
      assert.equal(lineNetTag({ ...line(1), net: NET_A }, store), "");
      assert.equal(lineNetTag(line(2), store), "");
    });
  });
});

// A network summary the way the overview reports it.
function net(id, label, idHex) {
  return {
    id, label, id_hex: idHex, status: "online", exit_enabled: false, proxy: id === NET_A, route_state: "direct", exits: [],
    members_voting: 1, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0,
  };
}

// A store whose slots with a key have a state (visible); the rest have none.
function setup(nets) {
  const store = createStore();
  store.nets = nets;
  for (const n of nets) {
    store.slots[n.id] = { id: n.id, brief: n, state: n.id_hex ? { network: { id_hex: n.id_hex } } : null, members: null, config: null, configRev: null, configMeta: null, bootId: "b1" };
  }
  return store;
}

// Network `id` is left: the slot no longer holds a key.
function leave(store, id) {
  store.nets = store.nets.filter((n) => n.id !== id);
  delete store.slots[id];
}
