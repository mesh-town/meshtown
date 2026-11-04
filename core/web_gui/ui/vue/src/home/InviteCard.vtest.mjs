/* The "Network invite" card: the invite is a secret, visible only in the card and only for its own network. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import { INVITE_NAME, openInviteDialog, SUGGESTED_NAME, submitInviteDialog } from "../testing/invite-dialog.mjs";
import InviteCard from "./InviteCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const SECRET = "mesh-invite2-SECRETSECRET";
const VALUE = '[aria-label="Invite to send"]';
const WARNING = "#invite-reach-warning";
// Warning texts as literals: the contract catches a typo.
const WARN_ISSUER_ONLY = "Only this computer can let them in — don't turn it off until the new member joins.";
const WARN_ISSUER_AND_ONE = "Better not to turn this computer off until the new member joins.";
let mounted;

beforeEach(() => { api.mockReset().mockResolvedValue({ invite: SECRET }); });
afterEach(() => mounted.wrapper.unmount());

describe("InviteCard", () => {
  it("submits the suggested name and shows the invite", async () => {
    const { wrapper } = mountInvite();
    const dialog = await openDialog(wrapper);
    expect(dialog.get(INVITE_NAME).element.value).toBe(SUGGESTED_NAME);
    expect(api).not.toHaveBeenCalledWith("GET", "/api/network/invite?for=" + SUGGESTED_NAME);
    await submitInviteDialog(dialog);
    expect(api).toHaveBeenCalledWith("GET", "/api/network/invite?for=" + SUGGESTED_NAME);
    expect(wrapper.get(VALUE).text()).toBe(SECRET);
  });

  it("id_hex changes — the string is empty", async () => {
    const { wrapper, store } = mountInvite();
    await click(wrapper);
    store.state = baseState({ network: { id_hex: "net-bbbb", label: "other" } });
    await flushPromises();
    expect(wrapper.get(VALUE).text()).toBe("");
  });

  it("before the click there is no string, the status is empty", () => {
    const { wrapper } = mountInvite();
    expect(wrapper.get(VALUE).text()).toBe("");
    expect(wrapper.get("#invite-status").attributes("role")).toBe("status");
  });

  it("the title is always \"Network invite\", without \"null\"", () => {
    const { wrapper } = mountInvite(baseState({ network: { id_hex: "net-aaaa", label: null } }));
    expect(wrapper.get("#home-invite-title").text()).toBe("Network invite");
  });

  it("a network with a label and several members — the same title, the network name in the counter", () => {
    const { wrapper } = mountInvite(baseState({ network: { id_hex: "net-aaaa", label: "family" } }));
    expect(wrapper.get("#home-invite-title").text()).toBe("Network invite");
    expect(wrapper.get("#home-members-count").text()).toMatch(/^Network "family" · /);
  });

  it("no network — no card", () => {
    const { wrapper } = mountInvite(baseState({ network: null }));
    expect(wrapper.find("#netkey-invite-btn").exists()).toBe(false);
  });

  it("the invite string never ends up in title attributes or in storage", async () => {
    const { wrapper } = mountInvite();
    await click(wrapper);
    expect(wrapper.html().split(SECRET).length - 1, "only in the <code> text").toBe(1);
    expect(JSON.stringify({ ...localStorage })).not.toContain("SECRETSECRET");
    expect(JSON.stringify({ ...sessionStorage })).not.toContain("SECRETSECRET");
  });
});

describe("InviteCard: the reach warning", () => {
  describe("before the request", () => {
    it("the element exists, is hidden and empty", () => {
      const { wrapper } = mountInvite();
      const warning = wrapper.get(WARNING);
      expect(warning.attributes("hidden"), "hidden").toBeDefined();
      expect(warning.text()).toBe("");
    });

    it("it is <p class=\"hint hint-warn\"> right after the invite string, without style and title", () => {
      const { wrapper } = mountInvite();
      const warning = wrapper.get(WARNING);
      expect(wrapper.get("#invite-value-wrap").element.nextElementSibling).toBe(warning.element);
      expect(warning.element.tagName).toBe("P");
      expect(warning.classes()).toEqual(expect.arrayContaining(["hint", "hint-warn"]));
      expect(warning.attributes("style")).toBeUndefined();
      expect(warning.attributes("title")).toBeUndefined();
    });
  });

  describe("the node replied reach = issuer_only", () => {
    it("the warning is visible, the text is \"Only this computer can let them in…\"", async () => {
      const { wrapper } = await clickInvite({ invite: SECRET, holders: 0, reach: "issuer_only" });
      const warning = wrapper.get(WARNING);
      expect(warning.attributes("hidden"), "visible").toBeUndefined();
      expect(warning.text()).toBe(WARN_ISSUER_ONLY);
    });

    it("the invite string is not repeated in the warning, it is single in the html", async () => {
      const { wrapper } = await clickInvite({ invite: SECRET, holders: 0, reach: "issuer_only" });
      expect(wrapper.get(WARNING).text()).not.toContain("SECRETSECRET");
      expect(wrapper.get(WARNING).html()).not.toContain("SECRETSECRET");
      expect(wrapper.html().split(SECRET).length - 1, "only in <code>").toBe(1);
      expect(wrapper.get(VALUE).text()).toBe(SECRET);
    });
  });

  describe("the node replied reach = issuer_and_one", () => {
    it("the warning is visible, the text is \"Better not to turn this computer off…\"", async () => {
      const { wrapper } = await clickInvite({ invite: SECRET, holders: 1, reach: "issuer_and_one" });
      const warning = wrapper.get(WARNING);
      expect(warning.attributes("hidden"), "visible").toBeUndefined();
      expect(warning.text()).toBe(WARN_ISSUER_AND_ONE);
    });
  });

  describe("no warning", () => {
    const quiet = [
      ["reach = spread", { invite: SECRET, holders: 2, reach: "spread" }],
      ["no reach field (old node)", { invite: SECRET }],
      ["no reach field, but holders present", { invite: SECRET, holders: 1 }],
      ["unknown reach value", { invite: SECRET, reach: "everyone" }],
    ];
    for (const [name, reply] of quiet) {
      it(name + " — hidden and empty, the invite is shown", async () => {
        const { wrapper } = await clickInvite(reply);
        expect(wrapper.get(WARNING).attributes("hidden"), "hidden").toBeDefined();
        expect(wrapper.get(WARNING).text()).toBe("");
        expect(wrapper.get(VALUE).text()).toBe(SECRET);
      });
    }

    it("the node refused — no invite and no warning", async () => {
      const { wrapper } = await clickInvite(new Error("busy"));
      expect(wrapper.get(WARNING).attributes("hidden"), "hidden").toBeDefined();
      expect(wrapper.get(VALUE).text()).toBe("");
    });
  });

  describe("repeated click", () => {
    it("issuer_only, then spread — the warning goes out", async () => {
      const { wrapper } = await clickInvite({ invite: SECRET, reach: "issuer_only" });
      api.mockResolvedValue({ invite: SECRET, reach: "spread" });
      await click(wrapper);
      expect(wrapper.get(WARNING).attributes("hidden"), "hidden").toBeDefined();
      expect(wrapper.get(WARNING).text()).toBe("");
    });

    it("issuer_only, then issuer_and_one — the text is replaced", async () => {
      const { wrapper } = await clickInvite({ invite: SECRET, reach: "issuer_only" });
      api.mockResolvedValue({ invite: SECRET, reach: "issuer_and_one" });
      await click(wrapper);
      expect(wrapper.get(WARNING).text()).toBe(WARN_ISSUER_AND_ONE);
    });
  });

  describe("the network changed", () => {
    it("the warning goes out along with the string", async () => {
      const { wrapper, store } = await clickInvite({ invite: SECRET, reach: "issuer_only" });
      store.state = baseState({ network: { id_hex: "net-bbbb", label: "other" } });
      await flushPromises();
      expect(wrapper.get(WARNING).attributes("hidden"), "hidden").toBeDefined();
      expect(wrapper.get(WARNING).text()).toBe("");
      expect(wrapper.get(VALUE).text()).toBe("");
    });
  });
});

function mountInvite(state = baseState({ network: { id_hex: "net-aaaa", label: "home" } })) {
  return (mounted = mountSection(InviteCard, { state }));
}

async function openDialog(wrapper) {
  api.mockResolvedValueOnce({ name: SUGGESTED_NAME });
  return openInviteDialog(wrapper);
}

async function click(wrapper) {
  await submitInviteDialog(await openDialog(wrapper));
}

// Mounting, the node's reply (`reply`: an object or an Error) and a click on "Invite a member". A refusal is for one call:
// the `beforeEach` above returns the mock itself, and vitest calls it as a finalizer (on a refusal the test would fail).
async function clickInvite(reply) {
  const view = mountInvite();
  const dialog = await openDialog(view.wrapper);
  if (reply instanceof Error) api.mockRejectedValueOnce(reply);
  else api.mockResolvedValue(reply);
  await submitInviteDialog(dialog);
  return view;
}

describe("InviteCard: for whom", () => {
  it("the label goes to the request as ?for= (trimmed, encoded)", async () => {
    const { wrapper } = mountInvite();
    const dialog = await openDialog(wrapper);
    await dialog.get(INVITE_NAME).setValue("  Anna's laptop ");
    await submitInviteDialog(dialog);
    expect(api).toHaveBeenCalledWith("GET", "/api/network/invite?for=Anna's%20laptop");
  });

  it("cancels without issuing an invite", async () => {
    const { wrapper } = mountInvite();
    const dialog = await openDialog(wrapper);
    expect(dialog.get(INVITE_NAME).attributes("maxlength")).toBe("64");
    await dialog.get('button[type="button"]').trigger("click");
    expect(wrapper.find("dialog[open]").exists()).toBe(false);
    expect(api.mock.calls).toEqual([["GET", "/api/network/invite-name"]]);
  });
});

describe("InviteCard: only you in the network", () => {
  const ALONE = "Only you in \"family\" so far. Invite the people you trust.";
  const FAMILY = { id_hex: "net-aaaa", label: "family" };
  const selfOnly = () => baseState().peers.filter((p) => p.self);
  const hidden = (wrapper, id) => wrapper.get(id).attributes("hidden") !== undefined;

  it("the counter calls to invite: Only you in \"family\" so far. Invite the people you trust.", () => {
    const { wrapper } = mountInvite(baseState({ network: FAMILY, peers: [] }));
    expect(wrapper.get("#home-members-count").text()).toBe(ALONE);
    expect(hidden(wrapper, "#home-members-count"), "the counter is visible").toBe(false);
  });

  it("the snapshot has only the own row — the same counter", () => {
    const { wrapper } = mountInvite(baseState({ network: FAMILY, peers: selfOnly() }));
    expect(wrapper.get("#home-members-count").text()).toBe(ALONE);
  });

  describe("the network status line", () => {
    const phases = [
      ["no transport in the snapshot (loading)", {}],
      ["the directory is not connected", { transport: { dht: { joined: false, nodes: 0, last_publish_sec: null } } }],
      ["the directory is connected", { transport: { dht: { joined: true, nodes: 3, last_publish_sec: null } } }],
    ];
    for (const [name, extra] of phases) {
      it(name + " — #home-net-status is hidden and empty: no \"Loading network data…\", no second call to invite", () => {
        const { wrapper } = mountInvite(baseState({ network: FAMILY, peers: selfOnly(), ...extra }));
        expect(hidden(wrapper, "#home-net-status"), "hidden").toBe(true);
        expect(wrapper.get("#home-net-status").text()).toBe("");
        expect(wrapper.text()).not.toContain("Loading network data");
        expect(wrapper.text()).not.toContain("Click \"Invite a member\"");
      });
    }

    it("another member is known but offline — the status line is still there (the card is not alone)", () => {
      const offline = { ...baseState().peers[0], online: false };
      const { wrapper } = mountInvite(baseState({ network: FAMILY, peers: [...selfOnly(), offline] }));
      expect(hidden(wrapper, "#home-net-status")).toBe(false);
    });
  });
});

describe("InviteCard: the form and the hint", () => {
  const HINT = "The invite is copied after you confirm the name. It works once, for 24 hours — send it to one person in a private message.";

  it("the hint is two sentences: when the invite is copied, and how long it lives", () => {
    const { wrapper } = mountInvite();
    const text = wrapper.get("#invite-hint").text();
    expect(text).toBe(HINT);
    expect(text.split(". ").length, "two sentences").toBe(2);
  });
});

describe("InviteCard: after the click", () => {
  beforeEach(() => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => {} }, configurable: true });
  });
  afterEach(() => Reflect.deleteProperty(navigator, "clipboard"));

  it("the status says \"✓ Invite copied — paste it into a private message\"", async () => {
    const { wrapper } = mountInvite();
    await click(wrapper);
    expect(wrapper.get("#invite-status").text()).toBe("✓ Invite copied — paste it into a private message");
    expect(wrapper.get("#invite-status").attributes("hidden")).toBeUndefined();
  });
});
