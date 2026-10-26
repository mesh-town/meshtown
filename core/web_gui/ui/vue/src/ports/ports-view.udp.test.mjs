/* Pure logic of Shared services for the two protocols (`ports-view.ts`): TCP and UDP ports with the same number are
 * different entries, a missing protocol reads as TCP, a TCP entry carries no `proto` field and a UDP one carries "udp". */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as view from "./ports-view.ts";

const TCP_DNS = { port: 53, name: "dns", for: "all" };
const UDP_DNS = { port: 53, proto: "udp", name: "dns", for: "all" };

describe("protoOf and portKey", () => {
  it("an entry without proto is TCP", () => {
    assert.equal(view.protoOf({}), "tcp");
  });

  it("an explicit tcp is TCP", () => {
    assert.equal(view.protoOf({ proto: "tcp" }), "tcp");
  });

  it("an explicit udp is UDP", () => {
    assert.equal(view.protoOf({ proto: "udp" }), "udp");
  });

  it("the TCP key is the bare number", () => {
    assert.equal(view.portKey("tcp", 8080), "8080");
  });

  it("the UDP key is the number with a /udp suffix", () => {
    assert.equal(view.portKey("udp", 53), "53/udp");
  });

  it("the same number gives different keys for the two protocols", () => {
    assert.notEqual(view.portKey("tcp", 53), view.portKey("udp", 53));
  });
});

describe("netRowKey", () => {
  it("a TCP row keeps the former key", () => {
    assert.equal(view.netRowKey({ node: "ab", port: 80 }), "ab/80");
  });

  it("an explicit tcp gives the same key as no proto", () => {
    assert.equal(view.netRowKey({ node: "ab", port: 80, proto: "tcp" }), "ab/80");
  });

  it("a UDP row ends with /udp", () => {
    assert.equal(view.netRowKey({ node: "ab", port: 53, proto: "udp" }), "ab/53/udp");
  });

  it("the slot goes first for both protocols", () => {
    assert.equal(view.netRowKey({ node: "ab", port: 53 }, "main"), "main/ab/53");
    assert.equal(view.netRowKey({ node: "ab", port: 53, proto: "udp" }, "main"), "main/ab/53/udp");
  });
});

describe("publishedAfterAdd", () => {
  describe("a UDP port", () => {
    it("is added with proto: udp", () => {
      const res = view.publishedAfterAdd([], 53, "dns", "all", "udp");
      assert.deepEqual(res.list, [UDP_DNS]);
    });

    it("keeps the node list as the audience", () => {
      const res = view.publishedAfterAdd([], 53, "dns", ["aa"], "udp");
      assert.deepEqual(res.list, [{ port: 53, proto: "udp", name: "dns", for: ["aa"] }]);
    });

    it("next to a TCP port with the same number gives two entries, the TCP one untouched", () => {
      const res = view.publishedAfterAdd([TCP_DNS], 53, "dns", "all", "udp");
      assert.deepEqual(res.list, [TCP_DNS, UDP_DNS]);
      assert.equal(res.error, undefined);
    });

    it("repeated: the UDP error text, no list", () => {
      const res = view.publishedAfterAdd([UDP_DNS], 53, "again", "all", "udp");
      assert.equal(res.error, "UDP port 53 is already published.");
      assert.equal(res.list, undefined);
    });

    it("an out-of-range number is the range error", () => {
      assert.equal(view.publishedAfterAdd([], 0, "", "all", "udp").error, "A port is a number from 1 to 65535.");
      assert.equal(view.publishedAfterAdd([], 65536, "", "all", "udp").error, "A port is a number from 1 to 65535.");
    });

    it("an empty node list is the no-nodes error", () => {
      const res = view.publishedAfterAdd([], 53, "", [], "udp");
      assert.equal(res.error, view.ADD_ERROR_NO_NODES);
      assert.equal(res.list, undefined);
    });
  });

  describe("a TCP port", () => {
    it("is added without a proto field when the protocol is not given", () => {
      const res = view.publishedAfterAdd([], 53, "dns", "all");
      assert.deepEqual(res.list, [TCP_DNS]);
      assert.equal("proto" in res.list[0], false, "a TCP entry has no proto field");
    });

    it("is added without a proto field when the protocol is tcp", () => {
      const res = view.publishedAfterAdd([], 53, "dns", "all", "tcp");
      assert.deepEqual(res.list, [TCP_DNS]);
      assert.equal("proto" in res.list[0], false, "a TCP entry has no proto field");
    });

    it("next to a UDP port with the same number gives two entries, the UDP one untouched", () => {
      const res = view.publishedAfterAdd([UDP_DNS], 53, "dns", "all", "tcp");
      assert.deepEqual(res.list, [UDP_DNS, TCP_DNS]);
    });

    it("repeated: the former error text", () => {
      const res = view.publishedAfterAdd([TCP_DNS, UDP_DNS], 53, "again", "all", "tcp");
      assert.equal(res.error, "Port 53 is already published.");
      assert.equal(res.list, undefined);
    });

    it("an entry from the node without proto counts as TCP", () => {
      const res = view.publishedAfterAdd([TCP_DNS], 53, "again", "all");
      assert.equal(res.error, "Port 53 is already published.");
    });
  });
});

describe("publishedAfterEdit", () => {
  it("a UDP edit replaces only the UDP entry", () => {
    const res = view.publishedAfterEdit([TCP_DNS, UDP_DNS], 53, "resolver", ["aa"], "udp");
    assert.deepEqual(res.list, [TCP_DNS, { port: 53, proto: "udp", name: "resolver", for: ["aa"] }]);
  });

  it("a TCP edit replaces only the TCP entry", () => {
    const res = view.publishedAfterEdit([TCP_DNS, UDP_DNS], 53, "resolver", ["aa"], "tcp");
    assert.deepEqual(res.list, [{ port: 53, name: "resolver", for: ["aa"] }, UDP_DNS]);
  });

  it("a TCP edit without the protocol argument leaves the UDP entry alone", () => {
    const res = view.publishedAfterEdit([UDP_DNS, TCP_DNS], 53, "resolver", "all");
    assert.deepEqual(res.list, [UDP_DNS, { port: 53, name: "resolver", for: "all" }]);
  });

  it("UDP not published (only TCP of that number): the UDP error text, no list", () => {
    const res = view.publishedAfterEdit([TCP_DNS], 53, "x", "all", "udp");
    assert.equal(res.error, "UDP port 53 is not published.");
    assert.equal(res.list, undefined);
  });

  it("TCP not published (only UDP of that number): the former error text, no list", () => {
    const res = view.publishedAfterEdit([UDP_DNS], 53, "x", "all", "tcp");
    assert.equal(res.error, "Port 53 is not published.");
    assert.equal(res.list, undefined);
  });

  it("an empty node list on a UDP entry is the no-nodes error", () => {
    const res = view.publishedAfterEdit([UDP_DNS], 53, "x", [], "udp");
    assert.equal(res.error, view.ADD_ERROR_NO_NODES);
    assert.equal(res.list, undefined);
  });
});

describe("publishedAfterRemove", () => {
  it("removing UDP keeps the TCP entry of the same number", () => {
    assert.deepEqual(view.publishedAfterRemove([TCP_DNS, UDP_DNS], 53, "udp"), [TCP_DNS]);
  });

  it("removing TCP keeps the UDP entry of the same number", () => {
    assert.deepEqual(view.publishedAfterRemove([TCP_DNS, UDP_DNS], 53, "tcp"), [UDP_DNS]);
  });

  it("without the protocol argument it removes TCP only", () => {
    assert.deepEqual(view.publishedAfterRemove([UDP_DNS, TCP_DNS], 53), [UDP_DNS]);
  });

  it("removing UDP that is not published changes nothing", () => {
    assert.deepEqual(view.publishedAfterRemove([TCP_DNS], 53, "udp"), [TCP_DNS]);
  });

  it("other ports stay", () => {
    const other = { port: 80, name: "site", for: "all" };
    assert.deepEqual(view.publishedAfterRemove([other, UDP_DNS], 53, "udp"), [other]);
  });
});

describe("openedAfterOpen", () => {
  it("a UDP entry has proto: udp and the given local number", () => {
    assert.deepEqual(view.openedAfterOpen([], "ab", 53, 5353, undefined, "udp"), [{ node: "ab", port: 53, proto: "udp", local: 5353 }]);
  });

  it("a TCP entry has no proto field", () => {
    const list = view.openedAfterOpen([], "ab", 53, 5353);
    assert.deepEqual(list, [{ node: "ab", port: 53, local: 5353 }]);
    assert.equal("proto" in list[0], false, "a TCP entry has no proto field");
  });

  it("an explicit tcp also gives no proto field", () => {
    const list = view.openedAfterOpen([], "ab", 53, 5353, undefined, "tcp");
    assert.equal("proto" in list[0], false, "a TCP entry has no proto field");
  });

  it("an empty UDP local number is the member's port plus 2000", () => {
    assert.deepEqual(view.openedAfterOpen([], "ab", 53, "", undefined, "udp"), [{ node: "ab", port: 53, proto: "udp", local: 2053 }]);
  });

  it("UDP next to an opened TCP of the same node and port is a second entry", () => {
    const tcp = { node: "ab", port: 53, local: 2053 };
    assert.deepEqual(view.openedAfterOpen([tcp], "ab", 53, 5353, undefined, "udp"), [tcp, { node: "ab", port: 53, proto: "udp", local: 5353 }]);
  });

  it("TCP next to an opened UDP of the same node and port is a second entry", () => {
    const udp = { node: "ab", port: 53, proto: "udp", local: 5353 };
    assert.deepEqual(view.openedAfterOpen([udp], "ab", 53, 2053), [udp, { node: "ab", port: 53, local: 2053 }]);
  });

  it("opening UDP again replaces only the UDP local number", () => {
    const tcp = { node: "ab", port: 53, local: 2053 };
    const udp = { node: "ab", port: 53, proto: "udp", local: 5353 };
    assert.deepEqual(view.openedAfterOpen([tcp, udp], "ab", 53, 6000, undefined, "udp"), [tcp, { node: "ab", port: 53, proto: "udp", local: 6000 }]);
  });

  it("opening TCP again replaces only the TCP local number", () => {
    const tcp = { node: "ab", port: 53, local: 2053 };
    const udp = { node: "ab", port: 53, proto: "udp", local: 5353 };
    assert.deepEqual(view.openedAfterOpen([tcp, udp], "ab", 53, 6000), [{ node: "ab", port: 53, local: 6000 }, udp]);
  });

  it("the same port of another node is a separate UDP entry", () => {
    const other = { node: "cd", port: 53, proto: "udp", local: 5353 };
    assert.deepEqual(view.openedAfterOpen([other], "ab", 53, 5354, undefined, "udp"), [other, { node: "ab", port: 53, proto: "udp", local: 5354 }]);
  });

  it("the default UDP number skips the taken ones", () => {
    const list = view.openedAfterOpen([], "ab", 53, "", new Set([2053]), "udp");
    assert.deepEqual(list, [{ node: "ab", port: 53, proto: "udp", local: 2054 }]);
  });
});

describe("openedAfterClose", () => {
  const tcp = { node: "ab", port: 53, local: 2053 };
  const udp = { node: "ab", port: 53, proto: "udp", local: 5353 };

  it("closing UDP keeps the TCP entry", () => {
    assert.deepEqual(view.openedAfterClose([tcp, udp], "ab", 53, "udp"), [tcp]);
  });

  it("closing TCP keeps the UDP entry", () => {
    assert.deepEqual(view.openedAfterClose([tcp, udp], "ab", 53, "tcp"), [udp]);
  });

  it("without the protocol argument it closes TCP only", () => {
    assert.deepEqual(view.openedAfterClose([udp, tcp], "ab", 53), [udp]);
  });

  it("closing UDP of another node keeps this node's UDP entry", () => {
    assert.deepEqual(view.openedAfterClose([udp], "cd", 53, "udp"), [udp]);
  });
});

describe("portStatusText", () => {
  it("a UDP own row says it is not checked, whatever the local state", () => {
    for (const local of ["ok", "refused", "unknown"]) {
      assert.equal(view.portStatusText({ port: 53, proto: "udp", name: "dns", local }), "not checked for UDP", "local = " + local);
    }
  });

  it("a TCP own row keeps its checked texts", () => {
    assert.equal(view.portStatusText({ port: 53, proto: "tcp", name: "dns", local: "ok" }), "responding");
    assert.equal(view.portStatusText({ port: 53, name: "dns", local: "refused" }), "not responding");
    assert.equal(view.portStatusText({ port: 53, name: "dns", local: "unknown" }), "not checked yet");
  });

  it("bind_failed keeps the busy text for a TCP row", () => {
    const row = netRow({ opened: { local: 2053, state: "bind_failed", error: "x" } });
    assert.equal(view.portStatusText(row), "port 2053 is busy — choose another number");
  });

  it("bind_failed keeps the busy text for a UDP row", () => {
    const row = netRow({ proto: "udp", opened: { local: 2053, state: "bind_failed", error: "x" } });
    assert.equal(view.portStatusText(row), "port 2053 is busy — choose another number");
  });
});

describe("lastErrorText", () => {
  const OPENED = (last_error) => ({ local: 2053, state: "listening", error: null, last_error });
  const TEXTS = {
    unsupported: "The member runs an old version: UDP is not supported",
    offline: "The member is offline",
    not_opened: "The member has not shared this port with you",
    no_service: "Nothing is listening on the member's port",
    timed_out: "The member did not answer in time",
    failed: "Couldn't reach the member's port",
  };

  describe("a UDP row", () => {
    for (const [code, text] of Object.entries(TEXTS)) {
      it("last_error " + code + " says: " + text, () => {
        assert.equal(view.lastErrorText(netRow({ proto: "udp", opened: OPENED(code) })), text);
      });
    }

    it("last_error null is empty", () => {
      assert.equal(view.lastErrorText(netRow({ proto: "udp", opened: OPENED(null) })), "");
    });

    it("a missing last_error is empty", () => {
      assert.equal(view.lastErrorText(netRow({ proto: "udp", opened: { local: 2053, state: "listening", error: null } })), "");
    });

    it("a port that is not opened has no text", () => {
      assert.equal(view.lastErrorText(netRow({ proto: "udp", opened: null })), "");
    });
  });

  describe("a TCP row", () => {
    it("unsupported reads as the generic failure", () => {
      assert.equal(view.lastErrorText(netRow({ opened: OPENED("unsupported") })), TEXTS.failed);
    });

    it("an explicit tcp with unsupported reads as the generic failure", () => {
      assert.equal(view.lastErrorText(netRow({ proto: "tcp", opened: OPENED("unsupported") })), TEXTS.failed);
    });

    for (const code of ["offline", "not_opened", "no_service", "timed_out", "failed"]) {
      it("last_error " + code + " says: " + TEXTS[code], () => {
        assert.equal(view.lastErrorText(netRow({ opened: OPENED(code) })), TEXTS[code]);
      });
    }

    it("last_error null is empty", () => {
      assert.equal(view.lastErrorText(netRow({ opened: OPENED(null) })), "");
    });

    it("a port that is not opened has no text", () => {
      assert.equal(view.lastErrorText(netRow({ opened: null })), "");
    });
  });
});

// ---------------------------------------------------------------------------------------------------------------------
// Helpers

function netRow(over) {
  return { node: "ab", node_name: "finch", online: true, port: 53, name: "dns", published: true, opened: null, web_url: null, ...over };
}
