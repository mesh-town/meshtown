/* The QR picture under the invite string in the "Network invite" card: drawn by the node, shown as an <img> with a data URI;
 * an invite too long for a code gets a note instead of a picture. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import InviteCard from "./InviteCard.vue";
import { HOME_INVITE_QR_ALT, HOME_INVITE_QR_TOO_LONG } from "./invite-qr-texts.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INVITE = "mesh-invite2-abcdefgh";
const SVG = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9 9"><rect width="9" height="9" fill="#fff"/><path d="M4 4h1v1H4V4" fill="#000"/></svg>';
const SVG_NEXT = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><path d="M0 0h1v1H0V0"/></svg>';
const DATA_PREFIX = "data:image/svg+xml;charset=utf-8,";
const PICTURE = "#invite-qr";
const NOTE = "#invite-qr-too-long";
const VALUE = '[aria-label="Invite to send"]';
const OTHER_NET = { id_hex: "net-bbbb", label: "other" };
const MANAGED_NO_RIGHT = { id_hex: "net-aaaa", label: "work", can_invite: false };
let mounted;

beforeEach(() => api.mockReset());
afterEach(() => mounted.wrapper.unmount());

describe("invite QR", () => {
  describe("before the click", () => {
    it("shows neither picture nor note", () => {
      const { wrapper } = mountInvite();
      expect(wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(wrapper.find(NOTE).exists(), "note").toBe(false);
    });
  });

  describe("the node sent an SVG", () => {
    it("shows the QR picture under the invite", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      const picture = view.wrapper.get(PICTURE);
      expect(picture.element.tagName).toBe("IMG");
      expect(decodedSrc(picture)).toBe(SVG);
      expect(picture.attributes("alt")).toBe(HOME_INVITE_QR_ALT);
      expect(view.wrapper.find(NOTE).exists(), "no note next to the picture").toBe(false);
      expect(view.wrapper.get(VALUE).text()).toBe(INVITE);
    });

    it("puts the picture after the reach warning, so the string and the warning stay together", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG, reach: "issuer_only" });
      const warning = view.wrapper.get("#invite-reach-warning").element;
      expect(warning.nextElementSibling).toBe(view.wrapper.get(PICTURE).element);
    });

    it("the SVG never becomes markup of the page: the picture is an <img>, not inline SVG", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      expect(view.wrapper.find("#home-invite svg").exists()).toBe(false);
    });
  });

  describe("the node sent qr = null (too long for a code)", () => {
    it("says the invite is too long for a QR code, the invite string stays visible", async () => {
      const view = await clickInvite({ invite: INVITE, qr: null });
      expect(view.wrapper.get(NOTE).text()).toBe(HOME_INVITE_QR_TOO_LONG);
      expect(view.wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(view.wrapper.get(VALUE).text()).toBe(INVITE);
    });
  });

  describe("an older node, no qr field", () => {
    it("treats it as too long: a note, no broken picture", async () => {
      const view = await clickInvite({ invite: INVITE });
      expect(view.wrapper.get(NOTE).text()).toBe(HOME_INVITE_QR_TOO_LONG);
      expect(view.wrapper.find(PICTURE).exists(), "picture").toBe(false);
    });
  });

  describe("the node refuses (409 busy)", () => {
    it("shows neither picture nor note", async () => {
      const view = mountInvite();
      api.mockRejectedValueOnce(new ApiError(409, "busy"));
      await click(view.wrapper);
      expect(view.wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(view.wrapper.find(NOTE).exists(), "note").toBe(false);
      expect(view.wrapper.get(VALUE).text()).toBe("");
    });
  });

  describe("a label is typed", () => {
    it("asks with the label and shows the picture", async () => {
      const view = mountInvite();
      api.mockResolvedValue({ invite: INVITE, qr: SVG });
      await view.wrapper.get("#invite-for-input").setValue("Anna");
      await click(view.wrapper);
      expect(api).toHaveBeenCalledWith("GET", "/api/network/invite?for=Anna");
      expect(decodedSrc(view.wrapper.get(PICTURE))).toBe(SVG);
    });
  });

  describe("the network changes", () => {
    it("drops the picture along with the string", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      view.store.state = baseState({ network: OTHER_NET });
      await flushPromises();
      expect(view.wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(view.wrapper.find(NOTE).exists(), "note").toBe(false);
      expect(view.wrapper.get(VALUE).text()).toBe("");
    });

    it("drops the note along with the string", async () => {
      const view = await clickInvite({ invite: INVITE, qr: null });
      view.store.state = baseState({ network: OTHER_NET });
      await flushPromises();
      expect(view.wrapper.find(NOTE).exists(), "note").toBe(false);
    });
  });

  describe("a second click", () => {
    it("replaces the picture with the new invite's", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      api.mockResolvedValue({ invite: INVITE + "x", qr: SVG_NEXT });
      await click(view.wrapper);
      expect(decodedSrc(view.wrapper.get(PICTURE))).toBe(SVG_NEXT);
      expect(view.wrapper.get(VALUE).text()).toBe(INVITE + "x");
    });

    it("with a too-long invite turns the picture into the note", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      api.mockResolvedValue({ invite: INVITE + "x", qr: null });
      await click(view.wrapper);
      expect(view.wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(view.wrapper.get(NOTE).text()).toBe(HOME_INVITE_QR_TOO_LONG);
    });

    it("a refusal after an issued invite leaves the string with its own picture", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      api.mockRejectedValueOnce(new ApiError(409, "busy"));
      await click(view.wrapper);
      expect(view.wrapper.get(VALUE).text()).toBe(INVITE);
      expect(decodedSrc(view.wrapper.get(PICTURE))).toBe(SVG);
    });
  });

  describe("the right to invite is taken away after the invite was issued", () => {
    it("keeps the picture with the invite text: only the string's own visibility decides", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG }, baseState({ network: { id_hex: "net-aaaa", label: "work" } }));
      view.store.state = baseState({ network: MANAGED_NO_RIGHT });
      await flushPromises();
      expect(view.wrapper.find("#netkey-invite-btn").exists(), "the button is replaced by the plaque").toBe(false);
      expect(view.wrapper.get(VALUE).text()).toBe(INVITE);
      expect(decodedSrc(view.wrapper.get(PICTURE))).toBe(SVG);
    });
  });

  describe("a member without the right to invite", () => {
    it("has no way to get an invite: neither picture nor note", () => {
      const { wrapper } = mountInvite(baseState({ network: MANAGED_NO_RIGHT }));
      expect(wrapper.find("#netkey-invite-btn").exists(), "button").toBe(false);
      expect(wrapper.find(PICTURE).exists(), "picture").toBe(false);
      expect(wrapper.find(NOTE).exists(), "note").toBe(false);
    });
  });

  describe("the picture and the secret", () => {
    it("the invite string is not in the picture's attributes, only in the <code> text", async () => {
      const view = await clickInvite({ invite: INVITE, qr: SVG });
      const picture = view.wrapper.get(PICTURE);
      expect(picture.attributes("alt")).not.toContain(INVITE);
      expect(picture.attributes("title")).toBeUndefined();
      expect(view.wrapper.html().split(INVITE).length - 1, "only in <code>").toBe(1);
    });
  });
});

function mountInvite(state = baseState({ network: { id_hex: "net-aaaa", label: "home" } })) {
  return (mounted = mountSection(InviteCard, { state }));
}

async function click(wrapper) {
  await wrapper.get("#netkey-invite-btn").trigger("click");
  await flushPromises();
}

// Mounting, the node's reply to the first click and the click itself.
async function clickInvite(reply, state) {
  const view = mountInvite(state);
  api.mockResolvedValue(reply);
  await click(view.wrapper);
  return view;
}

// The SVG the <img> carries, read back from its data URI.
function decodedSrc(picture) {
  const src = picture.attributes("src");
  expect(src.startsWith(DATA_PREFIX), "an SVG data URI").toBe(true);
  return decodeURIComponent(src.slice(DATA_PREFIX.length));
}
