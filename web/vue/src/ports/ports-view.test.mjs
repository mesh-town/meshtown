/* Pure logic of the "Ports" section (`ports-view.ts`): cases moved from the classic
 * `10-ports.test.mjs` (removed together with `10-ports.js`), plus the row key and the node label. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  ADD_ERROR_NO_NODES, LOCAL_PORT_SHIFT, audienceCandidates, audienceCandidatesWith, audienceOf, publishedAfterEdit, audienceText, shortId, configPorts, defaultLocalPort, netRowKey, nodeLabel, openErrorText, openedAfterClose, openedAfterOpen,
  portStatusText, publishedAfterAdd, publishedAfterRemove,
} from "./ports-view.ts";

describe("publishedAfterAdd", () => {
  it("adds an entry with for: all, does not change the source list", () => {
    const before = [{ port: 80, name: "site", for: "all" }];
    const res = publishedAfterAdd(before, 8080, "web");
    assert.deepEqual(res.list, [
      { port: 80, name: "site", for: "all" },
      { port: 8080, name: "web", for: "all" },
    ]);
    assert.equal(before.length, 1);
  });

  it("a repeated port — an error text, no list", () => {
    const res = publishedAfterAdd([{ port: 80, name: "", for: "all" }], 80, "more");
    assert.equal(res.error, "Port 80 is already published.");
    assert.equal(res.list, undefined);
  });

  it("port 0, 65536, NaN and fractional — an error text", () => {
    for (const bad of [0, 65536, NaN, 1.5]) {
      assert.equal(publishedAfterAdd([], bad, "").error, "A port is a number from 1 to 65535.");
    }
  });

  it("the bounds 1 and 65535 are allowed", () => {
    assert.equal(publishedAfterAdd([], 1, "").list.length, 1);
    assert.equal(publishedAfterAdd([], 65535, "").list.length, 1);
  });
});

describe("publishedAfterAdd: node list", () => {
  it("a node list — for with these ids", () => {
    const res = publishedAfterAdd([], 8081, "web", ["aa", "bb"]);
    assert.deepEqual(res.list, [{ port: 8081, name: "web", for: ["aa", "bb"] }]);
  });

  it("an empty list — ADD_ERROR_NO_NODES, no list", () => {
    const res = publishedAfterAdd([], 8081, "", []);
    assert.equal(res.error, ADD_ERROR_NO_NODES);
    assert.equal(res.list, undefined);
  });

  it("a repeated port is checked before an empty selection", () => {
    const res = publishedAfterAdd([{ port: 80, name: "", for: "all" }], 80, "", []);
    assert.equal(res.error, "Port 80 is already published.");
  });
});

const PEERS = [
  { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", self: false, online: true },
  { id: "aaaa1111", id_full: "A".repeat(64), name: "me", self: true, online: true },
  { id: "cccc3333", id_full: "C".repeat(64), name: "", self: false, online: false },
];

describe("audienceCandidates", () => {
  it("without self, peers order", () => {
    const c = audienceCandidates(PEERS);
    assert.deepEqual(c.map((x) => x.idFull), ["B".repeat(64), "C".repeat(64)]);
    assert.equal(c[0].label, "peer-one");
  });

  it("without a name — the short id (10 characters)", () => {
    const c = audienceCandidates(PEERS);
    assert.equal(c[1].label, "C".repeat(10));
    assert.equal(c[1].short, "C".repeat(10));
    assert.equal(shortId("D".repeat(64)), "D".repeat(10));
  });

  it("an offline node is present, online: false", () => {
    assert.equal(audienceCandidates(PEERS)[1].online, false);
  });

  it("null — empty", () => {
    assert.deepEqual(audienceCandidates(null), []);
    assert.deepEqual(audienceCandidates(undefined), []);
  });
});

describe("audienceOf", () => {
  const cands = audienceCandidates(PEERS);
  it("all — \"all\"", () => {
    assert.equal(audienceOf("all", ["B".repeat(64)], cands), "all");
  });

  it("nodes — only ids from the candidates, in their order", () => {
    const sel = ["C".repeat(64), "Z".repeat(64), "B".repeat(64)];
    assert.deepEqual(audienceOf("nodes", sel, cands), ["B".repeat(64), "C".repeat(64)]);
  });

  it("nodes without checkmarks — an empty array", () => {
    assert.deepEqual(audienceOf("nodes", [], cands), []);
  });
});

describe("audienceText", () => {
  it("no entry — \"—\"", () => {
    assert.equal(audienceText(undefined, PEERS), "—");
  });

  it("all — \"all members\"", () => {
    assert.equal(audienceText({ for: "all" }, PEERS), "all members");
  });

  it("[] — \"nobody\"", () => {
    assert.equal(audienceText({ for: [] }, PEERS), "nobody");
  });

  it("names comma-separated; not in peers or without a name — the short id", () => {
    const text = audienceText({ for: ["B".repeat(64), "C".repeat(64), "Z".repeat(64)] }, PEERS);
    assert.equal(text, "peer-one, " + "C".repeat(10) + ", " + "Z".repeat(10));
    assert.equal(audienceText({ for: ["B".repeat(64)] }, null), "B".repeat(10));
  });
});

describe("publishedAfterRemove", () => {
  it("removes only its own port", () => {
    const list = [{ port: 80, name: "a", for: "all" }, { port: 8080, name: "b", for: "all" }];
    assert.deepEqual(publishedAfterRemove(list, 80), [{ port: 8080, name: "b", for: "all" }]);
    assert.equal(list.length, 2);
  });
});

describe("defaultLocalPort", () => {
  it("port plus 2000: 80 → 2080", () => {
    assert.equal(LOCAL_PORT_SHIFT, 2000);
    assert.equal(defaultLocalPort(80), 2080);
  });

  it("edge: 63535 → 65535, 63536 → 63536 (does not fit — the member's number)", () => {
    assert.equal(defaultLocalPort(63535), 65535);
    assert.equal(defaultLocalPort(63536), 63536);
  });
});

describe("openedAfterOpen", () => {
  it("without local — port plus 2000", () => {
    assert.deepEqual(openedAfterOpen([], "ab", 8080, undefined), [{ node: "ab", port: 8080, local: 10080 }]);
    assert.deepEqual(openedAfterOpen([], "ab", 8080, ""), [{ node: "ab", port: 8080, local: 10080 }]);
  });

  it("own local — as given, as a string too", () => {
    assert.deepEqual(openedAfterOpen([], "ab", 8080, 9000), [{ node: "ab", port: 8080, local: 9000 }]);
    assert.deepEqual(openedAfterOpen([], "ab", 8080, "9001"), [{ node: "ab", port: 8080, local: 9001 }]);
  });

  it("a repeated pair does not duplicate but replaces local; the order and the other entries stay", () => {
    const list = [{ node: "cd", port: 22, local: 2022 }, { node: "ab", port: 8080, local: 80 }, { node: "ab", port: 22, local: 2023 }];
    const next = openedAfterOpen(list, "ab", 8080, 9000);
    assert.deepEqual(next, [{ node: "cd", port: 22, local: 2022 }, { node: "ab", port: 8080, local: 9000 }, { node: "ab", port: 22, local: 2023 }]);
    assert.equal(list[1].local, 80);
  });
});

describe("openedAfterClose", () => {
  it("removes only its own pair", () => {
    const list = [
      { node: "ab", port: 8080, local: 8080 },
      { node: "cd", port: 8080, local: 8081 },
      { node: "ab", port: 22, local: 22 },
    ];
    assert.deepEqual(openedAfterClose(list, "ab", 8080), [
      { node: "cd", port: 8080, local: 8081 },
      { node: "ab", port: 22, local: 22 },
    ]);
  });
});

describe("openErrorText", () => {
  it("\"Couldn't open: …\"", () => assert.equal(openErrorText("busy"), "Couldn't open: busy"));
});

describe("portStatusText", () => {
  it("own row: responding, not responding, not checked yet", () => {
    assert.equal(portStatusText({ port: 1, local: "ok" }), "responding");
    assert.equal(portStatusText({ port: 1, local: "refused" }), "not responding");
    assert.equal(portStatusText({ port: 1, local: "unknown" }), "not checked yet");
  });

  it("someone else's row: the author is offline", () => {
    assert.equal(portStatusText({ port: 1, online: false, opened: null }), "offline");
  });

  it("binding failed — \"port N\", even if the author is not online", () => {
    const opened = { local: 8080, state: "bind_failed", error: "busy" };
    const text = "port 8080 is busy — choose another number";
    assert.equal(portStatusText({ port: 1, online: true, opened }), text);
    assert.equal(portStatusText({ port: 1, online: false, opened }), text);
  });

  it("someone else's row online — \"online\"", () => {
    assert.equal(portStatusText({ port: 1, online: true, opened: null }), "online");
  });
});

describe("configPorts, netRowKey, nodeLabel", () => {
  it("no config or section — an empty list", () => {
    assert.deepEqual(configPorts(null, "published"), []);
    assert.deepEqual(configPorts({}, "opened"), []);
    assert.deepEqual(configPorts({ ports: { opened: [{ node: "a" }] } }, "opened"), [{ node: "a" }]);
  });

  it("row key — node and port via a slash", () => {
    assert.equal(netRowKey({ node: "ab", port: 80 }), "ab/80");
  });

  it("node label: the name or \"member <id>\"", () => {
    assert.equal(nodeLabel({ node: "ab", node_name: "finch" }), "finch");
    assert.equal(nodeLabel({ node: "ab", node_name: "  " }), "member ab");
    assert.equal(nodeLabel({ node: "ab" }), "member ab");
  });
});

describe("publishedAfterEdit", () => {
  const before = [{ port: 80, name: "site", for: "all" }, { port: 22, name: "ssh", for: ["aa"] }];

  it("replaces the name and for of an entry, the order and the other entries stay, the source list is intact", () => {
    const res = publishedAfterEdit(before, 22, "shell", ["bb", "cc"]);
    assert.deepEqual(res.list, [{ port: 80, name: "site", for: "all" }, { port: 22, name: "shell", for: ["bb", "cc"] }]);
    assert.deepEqual(before[1], { port: 22, name: "ssh", for: ["aa"] });
  });

  it("the port is not in the list — an error", () => {
    const res = publishedAfterEdit(before, 81, "x", "all");
    assert.equal(res.error, "Port 81 is not published.");
    assert.equal(res.list, undefined);
  });

  it("an empty node list — an error", () => {
    assert.equal(publishedAfterEdit(before, 80, "x", []).error, ADD_ERROR_NO_NODES);
  });
});

describe("audienceCandidatesWith", () => {
  const peers = [{ id_full: "AAAAAAAAAAAA", name: "one", online: true }, { id_full: "SELF", self: true, online: true }];

  it("appends a node that left, labeled with shortId and online: false, to the end", () => {
    const res = audienceCandidatesWith(peers, ["AAAAAAAAAAAA", "ZZZZZZZZZZZZZZ"]);
    assert.deepEqual(res.map((c) => c.idFull), ["AAAAAAAAAAAA", "ZZZZZZZZZZZZZZ"]);
    assert.deepEqual(res[1], { idFull: "ZZZZZZZZZZZZZZ", label: "ZZZZZZZZZZ", short: "ZZZZZZZZZZ", online: false });
  });
});
