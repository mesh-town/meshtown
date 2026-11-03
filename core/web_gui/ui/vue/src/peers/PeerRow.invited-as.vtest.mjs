/* The row of a member: the small note `(invited as "…")` after the name. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { baseState } from "../testing/section.mjs";
import { memberRow } from "../testing/members-fixtures.mjs";
import PeerRow from "./PeerRow.vue";

const peer = { id: "bbbb2222", id_full: "B".repeat(64), name: "peer-one", online: true, self: false };

const noteOf = (invited_as) => {
  const member = memberRow({ id_full: peer.id_full, name: "peer-one", invited_as });
  const wrapper = mount(PeerRow, { props: { peer, state: baseState(), chosen: [], member } });
  return wrapper.find(".peer-invited-as");
};

describe("PeerRow invited as", () => {
  it('shows (invited as "Anna") when it differs from the name', () => {
    expect(noteOf("Anna").text()).toBe('(invited as "Anna")');
  });

  it("hides it when equal to the name or null", () => {
    expect(noteOf("peer-one").exists()).toBe(false);
    expect(noteOf(null).exists()).toBe(false);
  });
});
