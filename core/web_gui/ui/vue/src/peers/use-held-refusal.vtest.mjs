/* `useHeldRefusal(read)` from `use-held-refusal.ts`: `read` is a getter of `{ names: string[] } | null`; returns a computed
 * ref with the same value, but not-null only once it has held for `ADMISSION_REFUSED_DELAY_MS` without a break; any break
 * drops it to `null` at once and the count starts over. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, ref } from "vue";
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

    it("the value turns into a null: null", async () => {
      const t = setup(refused());
      await vi.advanceTimersByTimeAsync(DELAY);
      await t.set(null);
      expect(t.held.value).toBeNull();
    });
  });

  describe("no refusal at all", () => {
    it("null: stays null after any time", async () => {
      const t = setup(null);
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

const refused = () => ({ names: ["peer-one"] });
const calm = () => null;

// Runs the composable inside a component (so unmounting is real); `held` is the computed ref it returns, `set` changes the source.
function setup(initial) {
  const source = ref(initial);
  const box = {};
  const Host = defineComponent({
    setup() {
      box.held = useHeldRefusal(() => source.value);
      return () => null;
    },
  });
  const wrapper = mount(Host);
  return { held: box.held, wrapper, set: async (next) => { source.value = next; await wrapper.vm.$nextTick(); } };
}
