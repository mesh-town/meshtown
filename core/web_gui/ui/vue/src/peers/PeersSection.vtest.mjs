import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { OLD_VERSION, OWN_VERSION, PEER_FULL, baseState, mountSection } from "../testing/section.mjs";
import PeersSection from "./PeersSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;
const mountPeers = (detail) => (mounted = mountSection(PeersSection, detail));

beforeEach(() => localStorage.clear());
afterEach(() => mounted && mounted.wrapper.unmount());

describe("PeersSection", () => {
  it("two members and \"You\" — three rows, \"You\" first", () => {
    const { wrapper } = mountPeers();
    const rows = wrapper.findAll("[role=list] > li");
    expect(rows).toHaveLength(3);
    expect(rows[0].text()).toContain("You: finch-unwary");
    expect(rows[1].text()).toContain("peer-one");
    expect(rows[2].text()).toContain("member cccc3333");
  });

  it("your own row — \"Others can open sites through you\", the refusing one — \"Not available as an exit\"", () => {
    const { wrapper } = mountPeers();
    const rows = wrapper.findAll("[role=list] > li");
    expect(rows[0].text()).toContain("Others can open sites through you");
    expect(rows[2].text()).toContain("Not available as an exit");
  });

  it("no state yet — \"Loading…\"", () => {
    const { wrapper } = mountPeers({ state: null });
    expect(wrapper.text()).toContain("Loading…");
  });

  it("no network — the NoNetwork message, no list", () => {
    const { wrapper } = mountPeers({ state: baseState({ network: null }) });
    expect(wrapper.find("[data-no-network-msg]").text()).toContain("You haven't joined a Mesh Town network yet");
    expect(wrapper.find("[role=list]").exists()).toBe(false);
  });

  it("click \"Use as exit\" — PUT /api/config with route.exits and \"Saved\"", async () => {
    api.mockResolvedValue({ config: { route: { exits: [PEER_FULL] } }, rev: 2 });
    const { wrapper } = mountPeers();
    await wrapper.get('[aria-label="Use as exit: peer-one"]').trigger("click");
    await flushPromises();
    expect(api).toHaveBeenCalledTimes(1);
    const [method, path, body] = api.mock.calls[0];
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.route.exits).toEqual([PEER_FULL]);
    expect(wrapper.get("[role=status]").text()).toBe("Saved");
  });

  it("422 refusal — the field error text in the status", async () => {
    const fields = [{ path: "route.exits", message: "the node doesn't allow exiting through itself" }];
    api.mockRejectedValue(new ApiError(422, "invalid", "invalid", { fields }));
    const { wrapper } = mountPeers();
    await wrapper.get('[aria-label="Use as exit: peer-one"]').trigger("click");
    await flushPromises();
    expect(wrapper.get("[role=status]").text()).toBe("the node doesn't allow exiting through itself");
  });
});

describe("PeersSection: member version in the row", () => {
  describe("version older than yours", () => {
    it("shown next to the name and the country, after the country", () => {
      const { wrapper } = mountPeers();
      const row = rowOf(wrapper, "peer-one");
      expect(row.get(".peer-who .peer-version").text()).toBe(OLD_VERSION);
      expect(isBefore(row.get(".peer-country"), row.get(".peer-version"))).toBe(true);
    });

    it("the \"older\" mark follows the version", () => {
      const { wrapper } = mountPeers();
      const row = rowOf(wrapper, "peer-one");
      expect(row.get(".peer-who .peer-older").text()).toBe("older version");
      expect(isBefore(row.get(".peer-version"), row.get(".peer-older"))).toBe(true);
    });

    it("the mark's hint names both versions and what to do", () => {
      const { wrapper } = mountPeers();
      const title = rowOf(wrapper, "peer-one").get(".peer-older").attributes("title");
      expect(title).toBe(`This member has Mesh Town ${OLD_VERSION}, you have ${OWN_VERSION}. Ask them to update Mesh Town.`);
    });

    it("the row is still one per member: name, country and code in place", () => {
      const { wrapper } = mountPeers();
      const who = rowOf(wrapper, "peer-one").get(".peer-who");
      expect(who.get(".peer-name").text()).toBe("peer-one");
      expect(who.get(".peer-country").text()).toBe("DE");
      expect(who.get(".peer-code").text()).toBe("bbbb2222");
    });
  });

  describe("version equals yours", () => {
    it("the version is shown, no \"older\" mark", () => {
      const { wrapper } = mountPeers(withPeerVersion(OWN_VERSION));
      const row = rowOf(wrapper, "peer-one");
      expect(row.get(".peer-version").text()).toBe(OWN_VERSION);
      expect(row.find(".peer-older").exists()).toBe(false);
    });
  });

  describe("version newer than yours", () => {
    it("the version is shown, no \"older\" mark", () => {
      const { wrapper } = mountPeers(withPeerVersion("0.2.0"));
      const row = rowOf(wrapper, "peer-one");
      expect(row.get(".peer-version").text()).toBe("0.2.0");
      expect(row.find(".peer-older").exists()).toBe(false);
    });
  });

  describe("version cannot be parsed (not three numbers)", () => {
    it("shown as is, no \"older\" mark", () => {
      const { wrapper } = mountPeers(withPeerVersion("abc"));
      const row = rowOf(wrapper, "peer-one");
      expect(row.get(".peer-version").text()).toBe("abc");
      expect(row.find(".peer-older").exists()).toBe(false);
    });
  });

  describe("an old node has no version (version: null)", () => {
    it("neither a version nor a mark", () => {
      const { wrapper } = mountPeers();
      const row = rowOf(wrapper, "member cccc3333");
      expect(row.find(".peer-version").exists()).toBe(false);
      expect(row.find(".peer-older").exists()).toBe(false);
    });

    it("the row text has no \"null\", \"undefined\" or empty parentheses", () => {
      const { wrapper } = mountPeers();
      const text = rowOf(wrapper, "member cccc3333").text();
      for (const junk of ["null", "undefined", "()", "[]"]) expect(text).not.toContain(junk);
    });

    it("there is no version field in the snapshot at all — the same", () => {
      const { wrapper } = mountPeers(withPeerVersion(undefined));
      const row = rowOf(wrapper, "peer-one");
      expect(row.find(".peer-version").exists()).toBe(false);
      expect(row.find(".peer-older").exists()).toBe(false);
      expect(row.text()).not.toContain("undefined");
    });
  });

  describe("your own row", () => {
    it("shows this build's version without the \"older\" mark", () => {
      const { wrapper } = mountPeers();
      const row = rowOf(wrapper, "You: finch-unwary");
      expect(row.get(".peer-version").text()).toBe(OWN_VERSION);
      expect(row.find(".peer-older").exists()).toBe(false);
    });
  });

  describe("your own version did not arrive in the snapshot (no node.version)", () => {
    it("member versions are visible, but \"older\" is written for nobody", () => {
      const state = baseState();
      const { wrapper } = mountPeers({ state: { ...state, node: { ...state.node, version: undefined } } });
      expect(wrapper.findAll(".peer-version")).toHaveLength(2);
      expect(wrapper.find(".peer-older").exists()).toBe(false);
    });
  });
});

// A list row by a text fragment (a name or the label of an unnamed one).
function rowOf(wrapper, fragment) {
  const row = wrapper.findAll("[role=list] > li").find((li) => li.text().includes(fragment));
  if (!row) throw new Error("no row with the text \"" + fragment + "\"");
  return row;
}

// The first element comes earlier in the document than the second.
function isBefore(first, second) {
  return Boolean(first.element.compareDocumentPosition(second.element) & Node.DOCUMENT_POSITION_FOLLOWING);
}

// A snapshot where peer-one has a different version (`undefined` — no field); the other rows are as in `baseState`.
function withPeerVersion(version) {
  const state = baseState();
  const peers = state.peers.map((p) => (p.name === "peer-one" ? { ...p, version } : p));
  return { state: { ...state, peers } };
}
