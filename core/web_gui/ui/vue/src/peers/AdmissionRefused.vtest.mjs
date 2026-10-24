/* The plate's 15-second rule: the timer restarts when the refusal goes away and comes back. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import AdmissionRefused from "./AdmissionRefused.vue";
import { ADMISSION_REFUSED_DELAY_MS } from "./peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => vi.useRealTimers());

const them = { side: "them", reason: "no", at_unix: 1 };
const refused = () => membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] });
const calm = () => membersReply({ rows: [selfRow(), peerOneRow({ online: true })] });

describe("AdmissionRefused", () => {
  it("the condition has to hold for 15 seconds in a row: a pause resets the count", async () => {
    const wrapper = mount(AdmissionRefused, { props: { members: refused(), net: "family" } });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS - 1000);
    await wrapper.setProps({ members: calm() });
    await wrapper.setProps({ members: refused() });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS - 1000);
    expect(wrapper.find("#admission-refused").exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.get("#admission-refused").text()).toContain('This computer can\'t connect to "family"');
    await wrapper.setProps({ members: calm() });
    expect(wrapper.find("#admission-refused").exists()).toBe(false);
    wrapper.unmount();
  });
});
