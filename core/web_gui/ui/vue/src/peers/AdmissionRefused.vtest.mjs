/* The plate's 15-second rule: the timer restarts when the refusal goes away and comes back. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import AdmissionRefused from "./AdmissionRefused.vue";
import { ADMISSION_REFUSED_DELAY_MS } from "./peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => vi.useRealTimers());

const them = { side: "them", reason: "no", at_unix: 1 };
const members = () => membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] });
const on = () => ({ refused: true, members: members(), net: "family" });
const off = () => ({ refused: false, members: members(), net: "family" });

describe("AdmissionRefused", () => {
  it("the condition has to hold for 15 seconds in a row: a pause resets the count", async () => {
    const wrapper = mount(AdmissionRefused, { props: on() });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS - 1000);
    await wrapper.setProps(off());
    await wrapper.setProps(on());
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS - 1000);
    expect(wrapper.find("#admission-refused").exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.get("#admission-refused").text()).toContain('This computer can\'t connect to "family"');
    await wrapper.setProps(off());
    expect(wrapper.find("#admission-refused").exists()).toBe(false);
    wrapper.unmount();
  });

  it("the flag on and no member list: the plate says that no member is online", async () => {
    const wrapper = mount(AdmissionRefused, { props: { refused: true, members: null, net: "family" } });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(wrapper.get("#admission-refused").text()).toContain('Members of "family" refuse to let it in, and no other member is online.');
    expect(wrapper.get("#admission-refused").text()).toContain("Ask a member");
    wrapper.unmount();
  });

  it("the flag off with a refusal in the list: no plate", async () => {
    const wrapper = mount(AdmissionRefused, { props: off() });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS * 2);
    expect(wrapper.find("#admission-refused").exists()).toBe(false);
    wrapper.unmount();
  });
});
