/* `members/PendingCard.vue`: the card of a computer that waits for members to confirm it. Props: `votes`, `need`. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import PendingCard from "./PendingCard.vue";

let wrapper;
afterEach(() => wrapper.unmount());

describe("PendingCard", () => {
  it("says how many members confirmed this computer", () => {
    wrapper = mount(PendingCard, { props: { votes: 1, need: 2 } });
    expect(wrapper.get("#members-pending-card h2").text()).toBe("Waiting to join the network");
    expect(wrapper.get("#members-pending-card-text").text()).toBe("This computer is waiting for network members to confirm it (1 of 2).");
  });
});
