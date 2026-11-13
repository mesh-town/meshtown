/* The member row of an overloaded exit: orange dot, the Overloaded badge and its tooltip. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { baseState, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { memberRow } from "../testing/members-fixtures.mjs";
import PeerRow from "./PeerRow.vue";

const peer = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, self: false, exit: true, overloaded: null };

const rowOf = (over, member = null) => mount(PeerRow, { props: { peer: { ...peer, ...over }, state: baseState(), chosen: [], member } });

describe("PeerRow overloaded", () => {
  it("peer row shows orange dot and Overloaded badge", () => {
    const tips = {
      limit: "It has reached its connection limit.",
      share: "Some members have reached their share of its connections.",
      memory: "It is low on memory and refuses new connections.",
      other: "It refuses some new connections.",
    };
    for (const [reason, tip] of Object.entries(tips)) {
      const w = rowOf({ overloaded: reason });
      expect(w.get(".dot").classes()).toContain("dot-orange");
      expect(w.get(".dot").classes()).not.toContain("dot-green");
      expect(w.get(".badge-busy").text()).toBe("Overloaded");
      expect(w.get(".badge-busy").attributes("title")).toBe(tip);
      expect(w.get(".peer-name").classes()).toEqual(["peer-name"]);
      expect(w.find("[data-exit-via]").exists()).toBe(true);
    }
    expect(rowOf({}).find(".badge-busy").exists()).toBe(false);
    expect(rowOf({}).get(".dot").classes()).toContain("dot-green");
  });

  it("offline peer is never orange", () => {
    const w = rowOf({ online: false, overloaded: "limit" });
    expect(w.get(".dot").classes()).toContain("dot-grey");
    expect(w.find(".badge-busy").exists()).toBe(false);
  });

  it("Overloaded badge stands next to a membership badge", () => {
    const member = memberRow({ id_full: PEER_FULL, name: "peer-one", blocked_by_me: true });
    const w = rowOf({ overloaded: "share" }, member);
    expect(w.find(".badge-busy").exists()).toBe(true);
    expect(w.findAll(".peer-badge").length).toBeGreaterThanOrEqual(2);
  });

  it("own row says this computer turns members away", () => {
    const w = rowOf({ self: true, id_full: SELF_FULL, overloaded: "limit" });
    expect(w.get(".dot").classes()).toContain("dot-orange");
    expect(w.get(".badge-busy").attributes("title")).toBe("This computer is turning members away.");
  });
});
