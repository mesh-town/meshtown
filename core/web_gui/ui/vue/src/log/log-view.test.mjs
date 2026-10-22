/* The pure logic of the Log section (`log-view.ts`): the `formatLogTime` and filter cases are moved from
 * the classic `11-log.test.mjs`; the rest — merging a poll, the ring, texts. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import {
  GAP_TEXT, chipTags, downloadUrl, emptyFeed, entryPasses, formatLogTime, mergePoll, pollErrorText, pollPath,
  ringHint, logRingMaxLines, trimRing, visibleEntries,
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
