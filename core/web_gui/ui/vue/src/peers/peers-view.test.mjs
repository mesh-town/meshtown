/* Pure logic of the "Members" section (`peers-view.ts`): the member rows
 * and their actions. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  countryCell,
  countryText,
  formatSeen,
  isOlderVersion,
  olderMark,
  olderTitle,
  orderedPeers,
  peerAction,
  peerMetaText,
  versionText,
} from "./peers-view.ts";

const ME = { self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "finch-unwary", country: "RU" };
const ONE = { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", country: "DE", online: true, path: "direct", rtt_ms: 30, exit: true };
const TWO = { id: "cccc3333", id_full: "C".repeat(64), name: "", country: null, online: false, last_seen_sec: 300, exit: false };
const STATE = { exit: { enabled: true } };
// A snapshot with this build's version: the member's version is compared with it ("older").
const STATE_V = { exit: { enabled: true }, node: { version: "0.1.0" } };

describe("formatSeen", () => {
  it("no timestamp — a dash", () => assert.equal(formatSeen(null), "—"));
  it("under a minute — \"just now\"", () => assert.equal(formatSeen(45), "just now"));
  it("a minute and more — in minutes", () => assert.equal(formatSeen(125), "2 min ago"));
  it("hours and days via agoText", () => {
    assert.equal(formatSeen(3600 * 5), "5 h ago");
    assert.equal(formatSeen(86400 * 3), "3 d ago");
  });
});

describe("countryCell", () => {
  it("country present + version — the country", () => assert.equal(countryCell({ country: "DE", version: "0.1.0" }), "DE"));
  it("no country + version — empty", () => assert.equal(countryCell({ country: null, version: "0.1.0" }), ""));
  it("no country + no version — a dash", () => assert.equal(countryCell({ country: null }), "—"));
});

describe("countryText", () => {
  it("a code present — the code as is", () => assert.equal(countryText({ country: "RU" }), "RU"));
  it("null — a dash", () => assert.equal(countryText({ country: null }), "—"));
  it("no field at all — a dash", () => assert.equal(countryText({}), "—"));
});

describe("peerMetaText", () => {
  it("\"You\" — whether sites can be opened through you", () => {
    assert.equal(peerMetaText(ME, STATE), "Others can open sites through you");
    assert.equal(peerMetaText(ME, { exit: { enabled: false } }), "Others can't open sites through you");
  });

  it("online — path and latency", () => {
    assert.equal(peerMetaText(ONE, STATE), "online · direct · 30 ms");
    assert.equal(peerMetaText({ ...ONE, path: "relay", rtt_ms: null }, STATE), "online · via relay server");
  });

  it("offline — \"last seen N min ago\", without a timestamp — just \"offline\"", () => {
    assert.equal(peerMetaText(TWO, STATE), "offline · last seen 5 min ago");
    assert.equal(peerMetaText({ ...TWO, last_seen_sec: null }, STATE), "offline");
    assert.equal(peerMetaText({ ...TWO, last_seen_sec: 7200 }, STATE), "offline · last seen 2 h ago");
    assert.equal(peerMetaText({ ...TWO, last_seen_sec: 10 }, STATE), "offline · last seen just now");
  });
});

describe("orderedPeers", () => {
  it("\"You\" first, even if it is not first in the snapshot; the order of the rest is kept", () => {
    assert.deepEqual(orderedPeers([ONE, TWO, ME]), [ME, ONE, TWO]);
  });

  it("no list — empty", () => assert.deepEqual(orderedPeers(undefined), []));
});

describe("peerAction", () => {
  it("\"You\" — a link to Home", () => assert.deepEqual(peerAction(ME, []), { kind: "self" }));
  it("an exit that is not chosen — a button", () => assert.deepEqual(peerAction(ONE, []), { kind: "button" }));

  it("the chosen exit is online — the \"✓\" mark", () => {
    assert.deepEqual(peerAction(ONE, [ONE.id_full]), { kind: "chosen-ok" });
  });

  it("chosen, but offline or not allowing — \"Chosen\", without \"✓\"", () => {
    assert.deepEqual(peerAction({ ...TWO, exit: null }, [TWO.id_full]), { kind: "chosen-down" });
    assert.deepEqual(peerAction({ ...ONE, exit: false }, [ONE.id_full]), { kind: "chosen-down" });
  });

  it("not allowing — a caption; exit unknown — no action", () => {
    assert.deepEqual(peerAction(TWO, []), { kind: "refused" });
    assert.deepEqual(peerAction({ ...TWO, exit: null }, []), { kind: "none" });
  });
});

describe("versionText", () => {
  it("a version is present — the string as is", () => {
    assert.equal(versionText({ version: "0.1.0" }), "0.1.0");
    assert.equal(versionText({ version: "0.2.0-rc1" }), "0.2.0-rc1");
  });

  it("null (old node), an empty string, no field, not a string — empty, without \"null\" and a dash", () => {
    assert.equal(versionText({ version: null }), "");
    assert.equal(versionText({ version: "" }), "");
    assert.equal(versionText({}), "");
    assert.equal(versionText({ version: 1 }), "");
  });
});

describe("isOlderVersion", () => {
  describe("both versions parse", () => {
    it("numbers are compared as numbers, not as strings: 0.9.0 is older than 0.10.0", () => {
      assert.equal(isOlderVersion("0.9.0", "0.10.0"), true);
    });

    it("0.10.0 is not older than 0.9.0", () => assert.equal(isOlderVersion("0.10.0", "0.9.0"), false));
    it("equal versions — not older", () => assert.equal(isOlderVersion("0.1.0", "0.1.0"), false));

    it("the first differing part from the left decides: 0.99.99 is older than 1.0.0, and 1.0.0 is not older than 0.99.99", () => {
      assert.equal(isOlderVersion("0.99.99", "1.0.0"), true);
      assert.equal(isOlderVersion("1.0.0", "0.99.99"), false);
    });

    it("a difference only in the last part: 1.2.3 is older than 1.2.4", () => {
      assert.equal(isOlderVersion("1.2.3", "1.2.4"), true);
      assert.equal(isOlderVersion("1.2.4", "1.2.3"), false);
    });
  });

  describe("after the first \"-\" or \"+\" — not counted", () => {
    it("1.2.3-rc1 and 1.2.3 — equal cores: not older in either direction", () => {
      assert.equal(isOlderVersion("1.2.3-rc1", "1.2.3"), false);
      assert.equal(isOlderVersion("1.2.3", "1.2.3-rc1"), false);
    });

    it("a build after \"+\" is not counted either", () => {
      assert.equal(isOlderVersion("1.2.3+build5", "1.2.3"), false);
      assert.equal(isOlderVersion("1.2.3", "1.2.3+build5"), false);
    });

    it("the tail does not save from a difference in the core: 1.2.3-rc9 is older than 1.2.4-rc1", () => {
      assert.equal(isOlderVersion("1.2.3-rc9", "1.2.4-rc1"), true);
      assert.equal(isOlderVersion("1.2.4-rc1", "1.2.3-rc9"), false);
    });
  });

  describe("version cannot be parsed — no comparison, no \"older\"", () => {
    // Each value is used for both the member and yourself; the other version is valid and clearly "greater"/"lesser".
    for (const bad of ["abc", "1.2", "1.2.3.4", "", "1.2.x", "1..3", "v1.2.3", "1.2.3.", "-1.2.3"]) {
      it(`«${bad}»`, () => {
        assert.equal(isOlderVersion(bad, "9.9.9"), false, "the member's");
        assert.equal(isOlderVersion("0.0.1", bad), false, "your own");
      });
    }

    it("\"1.2\" is not considered older than \"1.3.0\", although it is smaller by numbers", () => {
      assert.equal(isOlderVersion("1.2", "1.3.0"), false);
    });

    it("both invalid — not either", () => assert.equal(isOlderVersion("abc", "1.2"), false));
  });

  describe("not a string", () => {
    for (const bad of [null, undefined, 1, 1.5, {}, []]) {
      it(`${JSON.stringify(bad) ?? "undefined"}`, () => {
        assert.equal(isOlderVersion(bad, "9.9.9"), false, "the member's");
        assert.equal(isOlderVersion("0.0.1", bad), false, "your own");
      });
    }
  });
});

describe("olderMark and olderTitle", () => {
  const OLD = { ...ONE, version: "0.0.9" };
  const OLDER_TITLE = "This member has Mesh Town 0.0.9, you have 0.1.0. Ask them to update Mesh Town.";

  describe("a member with a strictly smaller version", () => {
    it("the mark is present", () => assert.equal(olderMark(OLD, STATE_V), true));
    it("the hint — verbatim, with both versions", () => assert.equal(olderTitle(OLD, STATE_V), OLDER_TITLE));
    it("0.9.0 against 0.10.0 — by numbers", () => {
      const state = { ...STATE_V, node: { version: "0.10.0" } };
      assert.equal(olderMark({ ...ONE, version: "0.9.0" }, state), true);
    });
  });

  describe("your own row", () => {
    it("with the build version — no mark and no hint", () => {
      const me = { ...ME, version: "0.1.0" };
      assert.equal(olderMark(me, STATE_V), false);
      assert.equal(olderTitle(me, STATE_V), "");
    });

    it("even with a version lower than the \"own\" one from node — never a mark", () => {
      assert.equal(olderMark({ ...ME, version: "0.0.1" }, STATE_V), false);
    });
  });

  describe("an old node without a version (null)", () => {
    it("no mark and no hint", () => {
      const old = { ...TWO, version: null };
      assert.equal(olderMark(old, STATE_V), false);
      assert.equal(olderTitle(old, STATE_V), "");
    });

    it("no version field at all — the same", () => assert.equal(olderMark(TWO, STATE_V), false));
  });

  describe("version equal or newer than your own", () => {
    it("equal — no", () => {
      assert.equal(olderMark({ ...ONE, version: "0.1.0" }, STATE_V), false);
      assert.equal(olderTitle({ ...ONE, version: "0.1.0" }, STATE_V), "");
    });

    it("newer — no", () => {
      assert.equal(olderMark({ ...ONE, version: "0.2.0" }, STATE_V), false);
      assert.equal(olderTitle({ ...ONE, version: "0.2.0" }, STATE_V), "");
    });
  });

  describe("version cannot be parsed or there is no own version", () => {
    it("the member's \"abc\" — no", () => assert.equal(olderMark({ ...ONE, version: "abc" }, STATE_V), false));

    it("your own version did not arrive in the snapshot — nothing to compare with, no mark", () => {
      assert.equal(olderMark(OLD, { ...STATE_V, node: {} }), false);
      assert.equal(olderMark(OLD, { ...STATE_V, node: { version: null } }), false);
    });
  });
});
