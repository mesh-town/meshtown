/* The member row's traffic line: shown with a tooltip for non-zero totals, absent for zeros. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { baseState, PEER_FULL } from "../testing/section.mjs";
import PeerRow from "./PeerRow.vue";

const peer = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", online: true, self: false, exit: true, overloaded: null };

const rowOf = (over) => mount(PeerRow, { props: { peer: { ...peer, ...over }, state: baseState(), chosen: [], member: null } });

describe("PeerRow traffic", () => {
  it("shows the line with a title for non-zero totals", () => {
    const w = rowOf({ through_peer_bytes: 1500, through_us_bytes: 0 });
    const line = w.get(".peer-traffic");
    expect(line.text()).toBe("You used their exit: 1.5 KB");
    expect(line.attributes("title")).toBe("Traffic in both directions since Mesh Town started on this device");
  });

  it("has no line for zeros or missing fields", () => {
    expect(rowOf({ through_peer_bytes: 0, through_us_bytes: 0 }).find(".peer-traffic").exists()).toBe(false);
    expect(rowOf({}).find(".peer-traffic").exists()).toBe(false);
  });
});
