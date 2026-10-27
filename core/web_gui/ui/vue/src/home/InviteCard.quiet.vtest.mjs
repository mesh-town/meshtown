/* `InviteCard quiet`: the invite button is a secondary one (used under the setup); without `quiet` it stays primary. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { baseState, mountSection } from "../testing/section.mjs";
import InviteCard from "./InviteCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const STATE = () => baseState({ network: { id_hex: "net-aaaa", label: "home" } });
let mounted;

afterEach(() => mounted.wrapper.unmount());

describe("InviteCard quiet", () => {
  it("quiet: the invite button is a plain .btn without .btn-primary", () => {
    const button = mountCard({ quiet: true }).wrapper.get("#netkey-invite-btn");
    expect(button.classes()).toContain("btn");
    expect(button.classes()).not.toContain("btn-primary");
  });

  it("not quiet: the invite button is primary", () => {
    const button = mountCard({}).wrapper.get("#netkey-invite-btn");
    expect(button.classes()).toContain("btn");
    expect(button.classes()).toContain("btn-primary");
  });

  it("quiet changes only the button: the card and the field are still there", () => {
    const { wrapper } = mountCard({ quiet: true });
    expect(wrapper.find("#home-invite").exists()).toBe(true);
    expect(wrapper.find("#invite-for-input").exists()).toBe(true);
  });
});

function mountCard(props) {
  return (mounted = mountSection(InviteCard, { state: STATE(), props }));
}
