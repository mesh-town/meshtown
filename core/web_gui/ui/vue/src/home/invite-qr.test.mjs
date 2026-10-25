/* The QR picture of an issued invite without Vue (`invite-actions.ts`): the data URI for an <img> and the `qr` field kept
 * next to the invite string. The SVG is the node's; the page only wraps it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { createInviteState, qrImageSrc, requestInvite, resetInviteIfNetChanged } from "./invite-actions.ts";

const INVITE = "mesh-invite2-abc";
const SVG = '<?xml version="1.0" standalone="yes"?><svg xmlns="http://www.w3.org/2000/svg" width="29" height="29" viewBox="0 0 29 29"><rect width="29" height="29" fill="#fff"/><path d="M4 4h1v1H4V4" fill="#000"/></svg>';
const SVG_NEXT = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><path d="M0 0h1v1H0V0"/></svg>';
const DATA_PREFIX = "data:image/svg+xml;charset=utf-8,";

describe("qrImageSrc", () => {
  it("turns an SVG into a data URI that decodes back to the same SVG", () => {
    const src = qrImageSrc(SVG);
    assert.ok(src.startsWith(DATA_PREFIX), "the SVG data URI prefix");
    assert.equal(decodeURIComponent(src.slice(DATA_PREFIX.length)), SVG);
  });

  it("keeps characters that would end a URI out of the payload", () => {
    const payload = qrImageSrc(SVG).slice(DATA_PREFIX.length);
    for (const bad of ["#", "<", ">", '"', " "]) assert.ok(!payload.includes(bad), `no raw ${JSON.stringify(bad)}`);
  });

  it("gives an empty string for no SVG", () => {
    assert.equal(qrImageSrc(""), "");
  });
});

describe("the qr field of the reply", () => {
  describe("a reply with an SVG", () => {
    it("is kept next to the invite string", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.text, INVITE);
      assert.equal(w.inv.qr, SVG);
    });
  });

  describe("a reply with qr = null (the invite is too long for a code)", () => {
    it("leaves the picture empty and the string shown", async () => {
      const w = setup({ invite: INVITE, qr: null });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.text, INVITE);
      assert.equal(w.inv.qr, "");
    });
  });

  describe("a reply without qr (an older node)", () => {
    it("leaves the picture empty and the string shown", async () => {
      const w = setup({ invite: INVITE });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.text, INVITE);
      assert.equal(w.inv.qr, "");
    });
  });

  describe("a second invite", () => {
    it("replaces the picture with the new one", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      w.reply({ invite: INVITE + "x", qr: SVG_NEXT });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.qr, SVG_NEXT);
    });

    it("with qr = null drops the previous picture: it never belongs to another string", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      w.reply({ invite: INVITE + "x", qr: null });
      await requestInvite(w.inv, w.store, w.deps);
      assert.equal(w.inv.text, INVITE + "x");
      assert.equal(w.inv.qr, "");
    });
  });

  describe("a refused request after an issued invite", () => {
    it("keeps the string and its picture together", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      w.fail(new ApiError(409, "busy"));
      assert.equal(await requestInvite(w.inv, w.store, w.deps), "failed");
      assert.equal(w.inv.text, INVITE);
      assert.equal(w.inv.qr, SVG);
    });
  });

  describe("the network changed", () => {
    it("the picture goes out along with the string", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      resetInviteIfNetChanged(w.inv, "net-b");
      assert.equal(w.inv.text, "");
      assert.equal(w.inv.qr, "");
    });

    it("the same network keeps the picture", async () => {
      const w = setup({ invite: INVITE, qr: SVG });
      await requestInvite(w.inv, w.store, w.deps);
      resetInviteIfNetChanged(w.inv, "net-a");
      assert.equal(w.inv.qr, SVG);
    });
  });

  describe("before any request", () => {
    it("there is no picture", () => {
      assert.equal(createInviteState().qr, "");
    });
  });
});

function setup(first) {
  const store = createStore();
  store.state = { network: { label: "home", id_hex: "net-a" }, peers: [] };
  let reply = first;
  let failure = null;
  const api = async () => {
    if (failure) throw failure;
    return reply;
  };
  return {
    inv: createInviteState(),
    store,
    deps: { api, clipboard: { writeText: async () => {} } },
    reply: (next) => { reply = next; failure = null; },
    fail: (error) => { failure = error; },
  };
}
