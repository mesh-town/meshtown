/* `useHeldRefusal(members)` from `use-held-refusal.ts`: `members` is a ref (or a getter) of `MembersReply | null | undefined`;
 * returns a computed ref `{ names: string[] } | null`: `admissionRefused(members)`, but not-null only once the refusal has held
 * for `ADMISSION_REFUSED_DELAY_MS` without a break; any break drops it to `null` at once and the count starts over. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, ref } from "vue";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "./peers-texts.ts";
import { useHeldRefusal } from "./use-held-refusal.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => vi.useRealTimers());

const DELAY = ADMISSION_REFUSED_DELAY_MS;

describe("useHeldRefusal", () => {
  describe("the refusal stands", () => {
    it("null right away and until the delay is over", async () => {
      const t = setup(refused());
      expect(t.held.value).toBeNull();
      await vi.advanceTimersByTimeAsync(DELAY - 1);
      expect(t.held.value).toBeNull();
    });

    it("the names of the refusing members once the delay is over", async () => {
      const t = setup(refused());
      await vi.advanceTimersByTimeAsync(DELAY);
      expect(t.held.value).toEqual({ names: ["peer-one"] });
    });

    it("works with a getter instead of a ref", async () => {
      const t = setup(refused(), { asGetter: true });
      await vi.advanceTimersByTimeAsync(DELAY);
      expect(t.held.value).toEqual({ names: ["peer-one"] });
    });
  });

  describe("the refusal goes away", () => {
    it("after it was shown: null at once", async () => {
      const t = setup(refused());
      await vi.advanceTimersByTimeAsync(DELAY);
      await t.set(calm());
      expect(t.held.value).toBeNull();
    });

    it("before the delay: the count starts over when it comes back", async () => {
      const t = setup(refused());
      await vi.advanceTimersByTimeAsync(DELAY - 1000);
      await t.set(calm());
      await t.set(refused());
      await vi.advanceTimersByTimeAsync(DELAY - 1000);
      expect(t.held.value).toBeNull();
      await vi.advanceTimersByTimeAsync(1000);
      expect(t.held.value).toEqual({ names: ["peer-one"] });
    });

    it("members become unknown (null): null", async () => {
      const t = setup(refused());
      await vi.advanceTimersByTimeAsync(DELAY);
      await t.set(null);
      expect(t.held.value).toBeNull();
    });
  });

  describe("no refusal at all", () => {
    it("members null: stays null after any time", async () => {
      const t = setup(null);
      await vi.advanceTimersByTimeAsync(DELAY * 2);
      expect(t.held.value).toBeNull();
    });

    it("somebody else is online: stays null after any time", async () => {
      const t = setup(calm());
      await vi.advanceTimersByTimeAsync(DELAY * 2);
      expect(t.held.value).toBeNull();
    });

    it("the computer is excluded: stays null (it has its own card)", async () => {
      const t = setup(membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: THEM })], me_excluded: true }));
      await vi.advanceTimersByTimeAsync(DELAY * 2);
      expect(t.held.value).toBeNull();
    });
  });

  it("unmounting stops the timer", async () => {
    const t = setup(refused());
    t.wrapper.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});

const THEM = { side: "them", reason: "no", at_unix: 1 };
const refused = () => membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: THEM })] });
const calm = () => membersReply({ rows: [selfRow(), peerOneRow({ online: true })] });

// Runs the composable inside a component (so unmounting is real); `held` is the computed ref it returns, `set` changes the members.
function setup(initial, { asGetter = false } = {}) {
  const members = ref(initial);
  const box = {};
  const Host = defineComponent({
    setup() {
      box.held = useHeldRefusal(asGetter ? () => members.value : members);
      return () => null;
    },
  });
  const wrapper = mount(Host);
  return { held: box.held, wrapper, set: async (next) => { members.value = next; await wrapper.vm.$nextTick(); } };
}
