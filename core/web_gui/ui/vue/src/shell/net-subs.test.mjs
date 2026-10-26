/* `net-subs.ts`: the three sections inside a network tab and how the address hash picks one. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { netSubViews, peersSub, pickNetSub } from "./net-subs.ts";

const NET_B = "k7q2xa";

describe("netSubViews", () => {
  describe("on a computer", () => {
    it("three sections in order: Members, Shared services (ports), Device settings", () => {
      assert.deepEqual(netSubViews(false), [
        { id: "members", label: "Members" },
        { id: "services", label: "Shared services (ports)" },
        { id: "device", label: "Device settings" },
      ]);
    });
  });

  describe("in the phone app", () => {
    it("has no services section but keeps members and device", () => {
      const ids = netSubViews(true).map((s) => s.id);
      assert.ok(!ids.includes("services"), "services is hidden");
      assert.ok(ids.includes("members"), "members stays");
      assert.ok(ids.includes("device"), "device stays");
    });

    it("two sections: Members and Device settings, services is hidden", () => {
      assert.deepEqual(netSubViews(true), [
        { id: "members", label: "Members" },
        { id: "device", label: "Device settings" },
      ]);
    });
  });
});

describe("peersSub", () => {
  describe("the hash names a section after the net", () => {
    it("services", () => assert.equal(peersSub(`#peers/${NET_B}/services`), "services"));
    it("device", () => assert.equal(peersSub("#peers/main/device"), "device"));
    it("members", () => assert.equal(peersSub(`#peers/${NET_B}/members`), "members"));
  });

  describe("no section in the hash", () => {
    it("a net without a tail — members", () => assert.equal(peersSub("#peers/main"), "members"));
    it("the bare #peers — members", () => assert.equal(peersSub("#peers"), "members"));
    it("add — members", () => assert.equal(peersSub("#peers/add"), "members"));
    it("an empty or missing hash — members", () => {
      assert.equal(peersSub(""), "members", "empty");
      assert.equal(peersSub(null), "members", "null");
      assert.equal(peersSub(undefined), "members", "undefined");
    });
  });

  describe("a hash that is not a section", () => {
    it("an unknown section after the net — members", () => {
      assert.equal(peersSub(`#peers/${NET_B}/bogus`), "members");
    });

    it("another top-level section — members", () => {
      assert.equal(peersSub("#log"), "members");
      assert.equal(peersSub("#home/services"), "members");
    });
  });

  describe("the old #ports link", () => {
    it("#ports → services", () => {
      assert.equal(peersSub("#ports"), "services");
    });
  });
});

describe("pickNetSub", () => {
  describe("on a computer", () => {
    it("keeps every wanted section", () => {
      for (const sub of ["members", "services", "device"]) assert.equal(pickNetSub(sub, false), sub, sub);
    });
  });

  describe("in the phone app", () => {
    it("services is hidden → members", () => {
      assert.equal(pickNetSub("services", true), "members");
    });

    it("members and device are kept", () => {
      assert.equal(pickNetSub("members", true), "members");
      assert.equal(pickNetSub("device", true), "device");
    });
  });
});
