/* "Exit via…": an overloaded candidate gets an orange " · overloaded" mark. */

import { afterEach, describe, expect, it } from "vitest";
import { baseState, mountSection, PEER_FULL } from "../testing/section.mjs";
import ExitCard from "./ExitCard.vue";

let mounted;
afterEach(() => mounted.wrapper.unmount());

function mountWith(over) {
  const base = baseState();
  const peers = base.peers.map((p) => (p.id_full === PEER_FULL ? { ...p, ...over } : p));
  mounted = mountSection(ExitCard, { state: baseState({ peers }) });
  return mounted.wrapper;
}

describe("ExitCard overloaded", () => {
  it("exit card marks overloaded candidates", () => {
    const busy = mountWith({ overloaded: "limit" });
    const meta = busy.get(`.exit-meta[data-id="${PEER_FULL}"]`);
    expect(meta.text()).toContain("· overloaded");
    expect(meta.get(".exit-busy").text()).toBe("· overloaded");
    mounted.wrapper.unmount();
    expect(mountWith({ overloaded: null }).find(".exit-busy").exists()).toBe(false);
    mounted.wrapper.unmount();
    expect(mountWith({ overloaded: "limit", online: false }).find(".exit-busy").exists()).toBe(false);
  });
});
