/* The "Pending invites" section: the open invites of a managed network, shown to admins only; the label is edited in
 * place on the admin's own invites. The API is a stub of `api()`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { invitesReply, openInvite } from "../testing/members-fixtures.mjs";
import PendingInvites from "./PendingInvites.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const OTHER = openInvite({ hash: "b".repeat(64), for_whom: "Boris", mine: false, issuer: "c".repeat(64), issuer_name: "anna" });
const MINE = openInvite();
let wrapper;

const open = async (props = {}) => {
  wrapper = mount(PendingInvites, { props: { slot: "main", meAdmin: true, ...props } });
  await flushPromises();
  return wrapper;
};
beforeEach(() => {
  api.mockResolvedValue(invitesReply([MINE, OTHER]));
});
afterEach(() => wrapper.unmount());

describe("PendingInvites", () => {
  it("shows open invites with their label, issuer and age", async () => {
    await open();
    const text = wrapper.get("#pending-invites").text();
    expect(api).toHaveBeenCalledWith("GET", "/api/network/invites");
    expect(wrapper.get("h2").text()).toBe("Pending invites");
    expect(text).toContain("by you");
    expect(text).toContain("by anna");
    expect(text).toContain("Boris");
    expect(text).toContain("issued");
  });

  it("edits the label of my invite and posts it", async () => {
    await open();
    const after = invitesReply([openInvite({ for_whom: "Anna K" }), OTHER]);
    api.mockResolvedValueOnce(after);
    await wrapper.get(`[data-invite="${MINE.hash}"] input`).setValue("Anna K");
    await wrapper.get(`[data-invite="${MINE.hash}"] form`).trigger("submit");
    await flushPromises();
    expect(api).toHaveBeenLastCalledWith("POST", "/api/network/invites/label", { hash: MINE.hash, for: "Anna K" });
    expect(wrapper.get(`[data-invite="${MINE.hash}"] input`).element.value).toBe("Anna K");
  });

  it("someone else's invite is read-only", async () => {
    await open();
    expect(wrapper.find(`[data-invite="${OTHER.hash}"] input`).exists()).toBe(false);
    expect(wrapper.get(`[data-invite="${OTHER.hash}"]`).text()).toContain("Boris");
  });

  it("an invite without a label says so", async () => {
    api.mockResolvedValue(invitesReply([openInvite({ for_whom: null, mine: false })]));
    await open();
    expect(wrapper.get("[data-invite]").text()).toContain("no name");
  });

  it("no invites — a quiet empty line", async () => {
    api.mockResolvedValue(invitesReply([]));
    await open();
    expect(wrapper.get("#pending-invites-empty").text()).toBe("No open invites");
  });

  it("not an admin: the section is not rendered and nothing is fetched", async () => {
    await open({ meAdmin: false });
    expect(wrapper.find("#pending-invites").exists()).toBe(false);
    expect(api).not.toHaveBeenCalled();
  });

  it("a 422 message is shown under the field", async () => {
    await open();
    api.mockRejectedValueOnce(new ApiError(422, "invalid", "x", { error: "invalid", message: "for: too long" }));
    await wrapper.get(`[data-invite="${MINE.hash}"] input`).setValue("x");
    await wrapper.get(`[data-invite="${MINE.hash}"] form`).trigger("submit");
    await flushPromises();
    expect(wrapper.get(`[data-invite="${MINE.hash}"] [role="alert"]`).text()).toBe("That label is not valid.");
  });

  it("409 not_issuer shows its message", async () => {
    await open();
    api.mockRejectedValueOnce(new ApiError(409, "not_issuer", "x", { error: "not_issuer", message: "x" }));
    await wrapper.get(`[data-invite="${MINE.hash}"] input`).setValue("y");
    await wrapper.get(`[data-invite="${MINE.hash}"] form`).trigger("submit");
    await flushPromises();
    expect(wrapper.get("[role=\"alert\"]").text()).toBe("Only the admin who made this invite can rename it.");
  });

  it("a second network uses /api/net/<slot>/network/invites", async () => {
    await open({ slot: "net-bbbb" });
    expect(api).toHaveBeenCalledWith("GET", "/api/net/net-bbbb/network/invites");
  });

  describe("kept invites", () => {
    const KEPT = openInvite({ kept: true });
    const FULL = "mesh-invite2-the-full-string";
    const QR_SVG = "<svg></svg>";
    const links = (hash) => wrapper.findAll(`[data-invite="${hash}"] .invite-share-link`);
    const answerText = () => api.mockImplementation(async (...a) => (a.length || console.log(new Error().stack), a[1].endsWith("/text") ? { invite: FULL, qr: QR_SVG } : invitesReply([KEPT, OTHER])));
    afterEach(() => Reflect.deleteProperty(navigator, "clipboard"));

    it("shows the two links, with an icon and text, only on rows that are kept", async () => {
      api.mockResolvedValue(invitesReply([KEPT, openInvite({ hash: "c".repeat(64), kept: false }), { ...OTHER, kept: false }]));
      await open();
      const own = links(KEPT.hash);
      expect(own.map((b) => b.text())).toEqual(["Copy invite", "Show QR code"]);
      expect(own.every((b) => b.find("svg").exists())).toBe(true);
      expect(links("c".repeat(64))).toHaveLength(0);
      expect(links(OTHER.hash)).toHaveLength(0);
    });

    it("\"Copy invite\" puts the full string on the clipboard and says Copied", async () => {
      const written = [];
      Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => void written.push(t) }, configurable: true });
      api.mockResolvedValue(invitesReply([KEPT]));
      await open();
      answerText();
      await links(KEPT.hash)[0].trigger("click");
      await flushPromises();
      expect(api).toHaveBeenCalledWith("POST", "/api/network/invites/text", { hash: KEPT.hash });
      expect(written).toEqual([FULL]);
      expect(wrapper.get(`[data-invite="${KEPT.hash}"] [role="status"]`).text()).toBe("Copied");
    });

    it("\"Show QR code\" opens the window on the QR picture; closing it forgets the string", async () => {
      api.mockResolvedValue(invitesReply([KEPT]));
      await open();
      answerText();
      await links(KEPT.hash)[1].trigger("click");
      await flushPromises();
      expect(wrapper.get("dialog").element.open).toBe(true);
      expect(wrapper.get("#invite-qr").attributes("src")).toContain("data:image/svg+xml");
      await wrapper.get("#invite-share-close").trigger("click");
      await flushPromises();
      expect(wrapper.html()).not.toContain(FULL);
    });

    it("a refused text shows the reason under the row and opens nothing", async () => {
      api.mockResolvedValue(invitesReply([KEPT]));
      await open();
      api.mockRejectedValueOnce(new ApiError(404, "not_found", "x", { error: "not_found", message: "x" }));
      await links(KEPT.hash)[1].trigger("click");
      await flushPromises();
      expect(wrapper.get(`[data-invite="${KEPT.hash}"] [role="alert"]`).text()).toContain("no longer be shown");
      expect(wrapper.get("dialog").element.open).toBe(false);
    });
  });
});
