/* The ring of an overloaded exit: the busy class, the dashes stay, the legend and the tab icon follow. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import RingMark from "./RingMark.vue";
import { ringMembers } from "./ring-members.ts";
import { ringLegend } from "./ring-text.ts";
import { faviconSvg } from "./ring-favicon.ts";
import { baseState, PEER_FULL } from "../testing/section.mjs";

function stateWith(over, path) {
  const base = baseState();
  const peers = base.peers.filter((p) => p.id_full === PEER_FULL || p.self).map((p) => (p.id_full === PEER_FULL ? { ...p, path } : p));
  const exit = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", country: "DE", exit: true, online: true, active: true, ...over };
  return baseState({ peers, route: { state: "exit", exits: [exit] } });
}

describe("ring of an overloaded exit", () => {
  it("ring arc of a busy exit has the busy class and keeps dashes", () => {
    const members = ringMembers(stateWith({ busy: true, overloaded: "limit" }, "relay"));
    const w = mount(RingMark, { props: { members, size: 112, thickness: 14 } });
    const arc = w.get("path.ring-arc.ring-via");
    expect(arc.classes()).toContain("ring-busy");
    expect(arc.classes()).toContain("ring-relayed");
    expect(arc.attributes("stroke-dasharray")).toBeTruthy();
    expect(ringLegend(members)[0].text).toBe("peer-one — your exit, overloaded — through another member");
    expect(faviconSvg(members, false)).toContain("#d97706");
    const calm = ringMembers(stateWith({ busy: false }, "direct"));
    expect(mount(RingMark, { props: { members: calm, size: 112, thickness: 14 } }).find(".ring-busy").exists()).toBe(false);
    expect(ringLegend(calm)[0].text).toBe("Your exit: peer-one");
    expect(faviconSvg(calm, false)).not.toContain("#d97706");
  });
});
