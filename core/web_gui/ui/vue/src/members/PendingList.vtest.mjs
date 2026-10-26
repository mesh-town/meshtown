/* `members/PendingList.vue`: the card with join requests. Props: `rows` (pending `MemberRow`s), `me` (my id_full).
 * Emits `pick(key, idFull)`: "admit" from the "Confirm…" button, other keys from the `⋯` menu. No rows - nothing is rendered. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import PendingList from "./PendingList.vue";
import { admissionInfo, pendingRow } from "../testing/members-fixtures.mjs";
import { SELF_FULL } from "../testing/section.mjs";

const ID = "E".repeat(64);
let wrapper;
afterEach(() => wrapper.unmount());

const setup = (rows) => (wrapper = mount(PendingList, { props: { rows, me: SELF_FULL }, attachTo: document.body }));

describe("PendingList", () => {
  it("shows the label and the badge", () => {
    setup([pendingRow({ admission: admissionInfo({ label: "Anna's laptop" }) })]);
    expect(wrapper.get("#members-pending-list h2").text()).toBe("Waiting to join the network");
    const row = wrapper.get("li");
    expect(row.get(".peer-name").text()).toBe("Anna's laptop");
    expect(row.get(".peer-badge").text()).toBe("Waiting to join");
    expect(row.get(".peer-code").text()).toBe("eeee5555");
  });

  it("a named node shows the name and the label on a separate line", () => {
    setup([pendingRow({ name: "laptop", admission: admissionInfo({ label: "Anna's laptop" }) })]);
    expect(wrapper.get(".peer-name").text()).toBe("laptop");
    expect(wrapper.findAll(".peer-line").map((l) => l.text())).toEqual(['Invited as "Anna\'s laptop"']);
  });

  it("without a name and a label the short id is shown", () => {
    setup([pendingRow()]);
    expect(wrapper.get(".peer-name").text()).toBe("eeee5555");
  });

  it("the Confirm button reports admit", async () => {
    setup([pendingRow()]);
    const button = wrapper.get('[data-member-action="admit"]');
    expect(button.text()).toBe("Confirm…");
    await button.trigger("click");
    expect(wrapper.emitted("pick")).toEqual([["admit", ID]]);
  });

  it("no rows - nothing is rendered", () => {
    setup([]);
    expect(wrapper.find("#members-pending-list").exists()).toBe(false);
  });
});
