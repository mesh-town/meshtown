/* `use-ports-actions.ts` for the two protocols: the PUT body of publish, edit, unpublish, open and close — a TCP entry has
 * no `proto` field, a UDP one has `proto: "udp"`, and an action on one protocol never touches the other. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { usePortsActions } from "./use-ports-actions.ts";

const TCP_DNS = { port: 53, name: "dns", for: "all" };
const UDP_DNS = { port: 53, proto: "udp", name: "dns", for: "all" };
const TCP_OPENED = { node: "ab", port: 53, local: 2053 };
const UDP_OPENED = { node: "ab", port: 53, proto: "udp", local: 5353 };

function setup({ published = [], opened = [], fail = null } = {}) {
  const store = createStore();
  store.config = { ports: { published, opened }, other: 1 };
  store.configRev = 3;
  const calls = [];
  const node = { fail };
  const api = async (method, path, body) => {
    const sent = JSON.parse(JSON.stringify(body));
    calls.push({ method, path, body: sent });
    if (node.fail) throw node.fail;
    return { config: sent.config, rev: body.rev + 1 };
  };
  const actions = usePortsActions(store, { api, busTarget: new EventTarget(), setTimer: () => {} });
  return { store, calls, actions, node };
}

const publishedSent = (w, n = 0) => w.calls[n].body.config.ports.published;
const openedSent = (w, n = 0) => w.calls[n].body.config.ports.opened;

describe("publish", () => {
  describe("a UDP port", () => {
    it("with a TCP port of the same number: PUT with both entries, the TCP one untouched", async () => {
      const w = setup({ published: [TCP_DNS] });
      assert.equal(await w.actions.publish(53, "dns", "all", "udp"), true);
      assert.deepEqual(publishedSent(w), [TCP_DNS, UDP_DNS]);
      assert.equal("proto" in publishedSent(w)[0], false, "the TCP entry still has no proto field");
    });

    it("on its own: the entry has proto: udp", async () => {
      const w = setup();
      assert.equal(await w.actions.publish(53, " dns ", "all", "udp"), true);
      assert.deepEqual(publishedSent(w), [UDP_DNS]);
      assert.equal(w.calls[0].method, "PUT");
      assert.equal(w.calls[0].path, "/api/config");
    });

    it("a string port from the form is a number in the entry", async () => {
      const w = setup();
      await w.actions.publish("53", "dns", "all", "udp");
      assert.deepEqual(publishedSent(w), [UDP_DNS]);
    });

    it("with a node list: for is that list", async () => {
      const w = setup();
      await w.actions.publish(53, "dns", ["aa"], "udp");
      assert.deepEqual(publishedSent(w), [{ port: 53, proto: "udp", name: "dns", for: ["aa"] }]);
    });

    it("repeated: the UDP form error, false, no request", async () => {
      const w = setup({ published: [UDP_DNS] });
      assert.equal(await w.actions.publish(53, "again", "all", "udp"), false);
      assert.equal(w.actions.addError.value, "UDP port 53 is already published.");
      assert.equal(w.calls.length, 0);
    });

    it("repeated next to a TCP port of the same number: still the UDP error", async () => {
      const w = setup({ published: [TCP_DNS, UDP_DNS] });
      assert.equal(await w.actions.publish(53, "again", "all", "udp"), false);
      assert.equal(w.actions.addError.value, "UDP port 53 is already published.");
      assert.equal(w.calls.length, 0);
    });

    it("an empty node list: the no-nodes error, no request", async () => {
      const w = setup();
      assert.equal(await w.actions.publish(53, "", [], "udp"), false);
      assert.notEqual(w.actions.addError.value, "");
      assert.equal(w.calls.length, 0);
    });

    it("a successful publish clears the earlier error", async () => {
      const w = setup({ published: [UDP_DNS] });
      await w.actions.publish(53, "", "all", "udp");
      assert.notEqual(w.actions.addError.value, "");
      await w.actions.publish(54, "", "all", "udp");
      assert.equal(w.actions.addError.value, "");
    });
  });

  describe("a TCP port", () => {
    it("without the protocol argument: no proto field, next to a UDP port of the same number", async () => {
      const w = setup({ published: [UDP_DNS] });
      assert.equal(await w.actions.publish(53, "dns", "all"), true);
      assert.deepEqual(publishedSent(w), [UDP_DNS, TCP_DNS]);
      assert.equal("proto" in publishedSent(w)[1], false, "the TCP entry has no proto field");
    });

    it("with the protocol tcp: no proto field", async () => {
      const w = setup();
      await w.actions.publish(8080, "web", "all", "tcp");
      assert.deepEqual(publishedSent(w), [{ port: 8080, name: "web", for: "all" }]);
      assert.equal("proto" in publishedSent(w)[0], false, "the TCP entry has no proto field");
    });

    it("repeated next to a UDP port: the former TCP error", async () => {
      const w = setup({ published: [TCP_DNS, UDP_DNS] });
      assert.equal(await w.actions.publish(53, "again", "all", "tcp"), false);
      assert.equal(w.actions.addError.value, "Port 53 is already published.");
      assert.equal(w.calls.length, 0);
    });
  });

  describe("node refusal", () => {
    it("a UDP publish is false and the status carries the error", async () => {
      const w = setup({ fail: new ApiError(500, "boom", "failure") });
      assert.equal(await w.actions.publish(53, "dns", "all", "udp"), false);
      assert.equal(w.actions.saveStatus.value.cls, "field-error-text");
      assert.match(w.actions.saveStatus.value.text, /^Couldn't save: /);
    });
  });
});

describe("unpublish", () => {
  it("UDP leaves the TCP entry of the same number", async () => {
    const w = setup({ published: [TCP_DNS, UDP_DNS] });
    assert.equal(await w.actions.unpublish(53, "udp"), true);
    assert.deepEqual(publishedSent(w), [TCP_DNS]);
  });

  it("TCP with the protocol argument leaves the UDP entry", async () => {
    const w = setup({ published: [TCP_DNS, UDP_DNS] });
    await w.actions.unpublish(53, "tcp");
    assert.deepEqual(publishedSent(w), [UDP_DNS]);
  });

  it("TCP without the protocol argument leaves the UDP entry", async () => {
    const w = setup({ published: [UDP_DNS, TCP_DNS] });
    await w.actions.unpublish(53);
    assert.deepEqual(publishedSent(w), [UDP_DNS]);
  });

  it("the other ports stay", async () => {
    const other = { port: 80, name: "site", for: "all" };
    const w = setup({ published: [other, UDP_DNS] });
    await w.actions.unpublish(53, "udp");
    assert.deepEqual(publishedSent(w), [other]);
  });
});

describe("edit", () => {
  it("UDP replaces only the UDP entry and keeps its proto", async () => {
    const w = setup({ published: [TCP_DNS, UDP_DNS] });
    assert.equal(await w.actions.edit(53, " resolver ", ["AA"], "udp"), true);
    assert.deepEqual(publishedSent(w), [TCP_DNS, { port: 53, proto: "udp", name: "resolver", for: ["AA"] }]);
    assert.equal(w.actions.editError.value, "");
  });

  it("TCP with the protocol argument replaces only the TCP entry", async () => {
    const w = setup({ published: [TCP_DNS, UDP_DNS] });
    await w.actions.edit(53, "resolver", ["AA"], "tcp");
    assert.deepEqual(publishedSent(w), [{ port: 53, name: "resolver", for: ["AA"] }, UDP_DNS]);
  });

  it("TCP without the protocol argument replaces only the TCP entry", async () => {
    const w = setup({ published: [UDP_DNS, TCP_DNS] });
    await w.actions.edit(53, "resolver", "all");
    assert.deepEqual(publishedSent(w), [UDP_DNS, { port: 53, name: "resolver", for: "all" }]);
  });

  it("UDP that is not published (only TCP of that number): the UDP error in editError, no request", async () => {
    const w = setup({ published: [TCP_DNS] });
    assert.equal(await w.actions.edit(53, "x", "all", "udp"), false);
    assert.equal(w.actions.editError.value, "UDP port 53 is not published.");
    assert.equal(w.calls.length, 0);
  });

  it("TCP that is not published (only UDP of that number): the former error in editError, no request", async () => {
    const w = setup({ published: [UDP_DNS] });
    assert.equal(await w.actions.edit(53, "x", "all"), false);
    assert.equal(w.actions.editError.value, "Port 53 is not published.");
    assert.equal(w.calls.length, 0);
  });

  it("an empty node list on a UDP entry: the no-nodes error in editError, no request", async () => {
    const w = setup({ published: [UDP_DNS] });
    assert.equal(await w.actions.edit(53, "x", [], "udp"), false);
    assert.notEqual(w.actions.editError.value, "");
    assert.equal(w.actions.addError.value, "");
    assert.equal(w.calls.length, 0);
  });

  it("node refusal of a UDP edit lands in editError", async () => {
    const w = setup({ published: [UDP_DNS], fail: new ApiError(500, "boom", "disk is full") });
    assert.equal(await w.actions.edit(53, "x", "all", "udp"), false);
    assert.match(w.actions.editError.value, /^Couldn't save: /);
  });
});

describe("openNetPort", () => {
  it("UDP with its own number: the entry has proto: udp", async () => {
    const w = setup();
    assert.equal(await w.actions.openNetPort("main", "ab", 53, 5353, "udp"), true);
    assert.deepEqual(openedSent(w), [UDP_OPENED]);
  });

  it("UDP without a number: the member's port plus 2000", async () => {
    const w = setup();
    await w.actions.openNetPort("main", "ab", 53, "", "udp");
    assert.deepEqual(openedSent(w), [{ node: "ab", port: 53, proto: "udp", local: 2053 }]);
  });

  it("TCP without the protocol argument: no proto field", async () => {
    const w = setup();
    await w.actions.openNetPort("main", "ab", 53, 2053);
    assert.deepEqual(openedSent(w), [TCP_OPENED]);
    assert.equal("proto" in openedSent(w)[0], false, "the TCP entry has no proto field");
  });

  it("TCP with the protocol tcp: no proto field", async () => {
    const w = setup();
    await w.actions.openNetPort("main", "ab", 53, 2053, "tcp");
    assert.equal("proto" in openedSent(w)[0], false, "the TCP entry has no proto field");
  });

  it("UDP next to an opened TCP of the same node and port: two entries", async () => {
    const w = setup({ opened: [TCP_OPENED] });
    await w.actions.openNetPort("main", "ab", 53, 5353, "udp");
    assert.deepEqual(openedSent(w), [TCP_OPENED, UDP_OPENED]);
  });

  it("opening UDP again changes only the UDP local number", async () => {
    const w = setup({ opened: [TCP_OPENED, UDP_OPENED] });
    await w.actions.openNetPort("main", "ab", 53, 6000, "udp");
    assert.deepEqual(openedSent(w), [TCP_OPENED, { ...UDP_OPENED, local: 6000 }]);
  });

  it("opening TCP again changes only the TCP local number", async () => {
    const w = setup({ opened: [TCP_OPENED, UDP_OPENED] });
    await w.actions.openNetPort("main", "ab", 53, 6000);
    assert.deepEqual(openedSent(w), [{ ...TCP_OPENED, local: 6000 }, UDP_OPENED]);
  });
});

describe("closeNetPort", () => {
  it("UDP leaves the TCP entry of the same node and port", async () => {
    const w = setup({ opened: [TCP_OPENED, UDP_OPENED] });
    assert.equal(await w.actions.closeNetPort("main", "ab", 53, "udp"), true);
    assert.deepEqual(openedSent(w), [TCP_OPENED]);
  });

  it("TCP with the protocol argument leaves the UDP entry", async () => {
    const w = setup({ opened: [TCP_OPENED, UDP_OPENED] });
    await w.actions.closeNetPort("main", "ab", 53, "tcp");
    assert.deepEqual(openedSent(w), [UDP_OPENED]);
  });

  it("TCP without the protocol argument leaves the UDP entry", async () => {
    const w = setup({ opened: [UDP_OPENED, TCP_OPENED] });
    await w.actions.closeNetPort("main", "ab", 53);
    assert.deepEqual(openedSent(w), [UDP_OPENED]);
  });
});

describe("openErrors", () => {
  const INVALID = new ApiError(422, "invalid", "invalid", { fields: [{ path: "ports.opened", message: "Local port 5353 is already in use" }] });

  it("a refused UDP open is recorded under the UDP row key only", async () => {
    const w = setup({ fail: INVALID });
    assert.equal(await w.actions.openNetPort("main", "ab", 53, 5353, "udp"), false);
    assert.deepEqual(w.actions.openErrors.value, { "ab/53/udp": "Local port 5353 is already in use" });
  });

  it("a refused TCP open is recorded under the TCP row key only", async () => {
    const w = setup({ fail: INVALID });
    await w.actions.openNetPort("main", "ab", 53, 2053);
    assert.deepEqual(Object.keys(w.actions.openErrors.value), ["ab/53"]);
  });

  it("the refusal of one protocol does not clear the other's", async () => {
    const w = setup({ fail: INVALID });
    await w.actions.openNetPort("main", "ab", 53, 2053);
    await w.actions.openNetPort("main", "ab", 53, 5353, "udp");
    assert.deepEqual(Object.keys(w.actions.openErrors.value).sort(), ["ab/53", "ab/53/udp"]);
  });

  it("a successful UDP open clears the UDP error and keeps the TCP one", async () => {
    const w = setup({ fail: INVALID });
    await w.actions.openNetPort("main", "ab", 53, 2053);
    await w.actions.openNetPort("main", "ab", 53, 5353, "udp");
    w.node.fail = null;
    assert.equal(await w.actions.openNetPort("main", "ab", 53, 5353, "udp"), true);
    assert.deepEqual(Object.keys(w.actions.openErrors.value), ["ab/53"]);
  });

  it("a successful UDP close clears an earlier UDP error", async () => {
    const w = setup({ fail: INVALID });
    await w.actions.openNetPort("main", "ab", 53, 5353, "udp");
    assert.deepEqual(Object.keys(w.actions.openErrors.value), ["ab/53/udp"]);
    w.node.fail = null;
    assert.equal(await w.actions.closeNetPort("main", "ab", 53, "udp"), true);
    assert.deepEqual(w.actions.openErrors.value, {});
  });
});
