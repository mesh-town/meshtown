/* The window after "Create invite": a text choice (the string and "Copy") and a QR choice (the picture or a "too long" note),
 * the steps for the new member, the reach warning, the icons. */

import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import InviteShareDialog from "./InviteShareDialog.vue";
import { HOME_INVITE_QR_TOO_LONG } from "./invite-qr-texts.ts";
import { UPDATE_DOWNLOAD_URL } from "./update-texts.ts";

const INVITE = "mesh-invite2-abcdefgh";
const QR = "data:image/svg+xml;charset=utf-8,%3Csvg%3E%3C%2Fsvg%3E";
const BY_HAND = "Select and copy it by hand";
let wrapper;

afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "clipboard");
});

function mountDialog(props = {}) {
  wrapper = mount(InviteShareDialog, { props: { text: INVITE, qr: QR, name: "Anna", warning: "", ...props }, attachTo: document.body });
  return wrapper;
}

async function opened(props) {
  const w = mountDialog(props);
  w.vm.open();
  await flushPromises();
  return w;
}

describe("InviteShareDialog", () => {
  it("opens as a modal titled with the name", async () => {
    const w = await opened();
    expect(w.get("dialog").element.open).toBe(true);
    expect(w.get("h2").text()).toBe("Invite for Anna");
  });

  it("shows the install link and the steps for the new member", async () => {
    const w = await opened();
    expect(w.get("ol a").attributes("href")).toBe(UPDATE_DOWNLOAD_URL);
    expect(w.get("ol").text()).toContain("Open it and choose to join a network.");
    expect(w.get("ol").text()).toContain("Paste the invite into “Invite you received”");
    expect(w.text()).toContain("Works once, for 24 hours. Send it only to this person.");
  });

  it("shows the string by default, not the picture", async () => {
    const w = await opened();
    expect(w.get("#invite-value").text()).toBe(INVITE);
    expect(w.find("#invite-qr").exists()).toBe(false);
  });

  it("\"Send as QR code\" shows an <img> with the QR as its src and the scan step", async () => {
    const w = await opened();
    await w.get("#invite-share-qr-btn").trigger("click");
    expect(w.get("img#invite-qr").attributes("src")).toBe(QR);
    expect(w.find("#invite-value").exists()).toBe(false);
    expect(w.get("ol").text()).toContain("Scan QR");
  });

  it("without a QR the QR choice says the invite is too long", async () => {
    const w = await opened({ qr: "" });
    await w.get("#invite-share-qr-btn").trigger("click");
    expect(w.find("img").exists()).toBe(false);
    expect(w.get("#invite-qr-too-long").text()).toBe(HOME_INVITE_QR_TOO_LONG);
    expect(w.text()).toContain("too long");
  });

  it("\"Copy\" writes the full string to the clipboard and says Copied", async () => {
    const written = [];
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => void written.push(t) }, configurable: true });
    const w = await opened();
    await w.get("#invite-share-copy").trigger("click");
    await flushPromises();
    expect(written).toEqual([INVITE]);
    expect(w.get("#invite-share-copy-status").text()).toBe("Copied");
  });

  it("when the clipboard refuses, asks to copy by hand", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => { throw new Error("denied"); } }, configurable: true });
    const w = await opened();
    await w.get("#invite-share-copy").trigger("click");
    await flushPromises();
    expect(w.get("#invite-share-copy-status").text()).toBe(BY_HAND);
  });

  it("both choices have a hidden SVG icon and a text label", async () => {
    const w = await opened();
    for (const [id, label] of [["#invite-share-text-btn", "Send as text"], ["#invite-share-qr-btn", "Send as QR code"]]) {
      const button = w.get(id);
      expect(button.get("svg").attributes("aria-hidden")).toBe("true");
      expect(button.text()).toBe(label);
    }
  });

  it("shows the reach warning when there is one", async () => {
    const w = await opened({ warning: "Keep this computer on." });
    expect(w.get("#invite-reach-warning").text()).toBe("Keep this computer on.");
    expect(w.get("#invite-reach-warning").attributes("hidden")).toBeUndefined();
  });

  it("reopens on the text choice", async () => {
    const w = await opened();
    await w.get("#invite-share-qr-btn").trigger("click");
    w.vm.close();
    w.vm.open();
    await flushPromises();
    expect(w.find("#invite-value").exists()).toBe(true);
  });

  it("closes when the invite string goes away", async () => {
    const w = await opened();
    await w.setProps({ text: "", qr: "" });
    expect(w.get("dialog").element.open).toBe(false);
  });

  it("\"Done\" closes it", async () => {
    const w = await opened();
    await w.get("#invite-share-close").trigger("click");
    expect(w.get("dialog").element.open).toBe(false);
  });
});
