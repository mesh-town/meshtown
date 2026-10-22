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
  it("shows the label, the badge and \"Confirmed by 1 of 2\"", () => {
    setup([pendingRow({ admission: admissionInfo({ label: "Anna's laptop" }) })]);
    expect(wrapper.get("#members-pending-list h2").text()).toBe("Waiting to join the network");
    const row = wrapper.get("li");
    expect(row.get(".peer-name").text()).toBe("Anna's laptop");
    expect(row.get(".peer-badge").text()).toBe("Waiting to join");
    expect(row.get(".peer-line").text()).toBe("Confirmed by 1 of 2");
    expect(row.get(".peer-code").text()).toBe("eeee5555");
  });

  it("a named node shows the name and the label on a separate line", () => {
    setup([pendingRow({ name: "laptop", admission: admissionInfo({ label: "Anna's laptop" }) })]);
    expect(wrapper.get(".peer-name").text()).toBe("laptop");
    expect(wrapper.findAll(".peer-line").map((l) => l.text())).toEqual(['Confirmed by 1 of 2', 'Invited as "Anna\'s laptop"']);
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

  it("no button when I have already confirmed; the menu withdraws", async () => {
    setup([pendingRow({ admission: admissionInfo({ mine: true }) })]);
    expect(wrapper.find('[data-member-action="admit"]').exists()).toBe(false);
    await wrapper.get('button[data-member-menu="' + ID + '"]').trigger("click");
    await wrapper.get('[data-member-item="unadmit"]').trigger("click");
    expect(wrapper.emitted("pick")).toEqual([["unadmit", ID]]);
  });

  it("no rows - nothing is rendered", () => {
    setup([]);
    expect(wrapper.find("#members-pending-list").exists()).toBe(false);
  });
});
