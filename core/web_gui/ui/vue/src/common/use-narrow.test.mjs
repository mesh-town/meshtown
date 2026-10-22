/* `useNarrow`: a ref that follows the narrow-screen media query, subscribes to "change" and unsubscribes on unmount. */

import { describe, expect, it, vi } from "vitest";
import { withSetup } from "../testing/with-setup.mjs";
import { stubNarrow } from "../testing/narrow-media.mjs";
import { useNarrow } from "./use-narrow.ts";

describe("useNarrow", () => {
  describe("the window matches the narrow query", () => {
    it("is true", () => {
      stubNarrow(true);
      const w = withSetup(() => useNarrow());
      expect(w.result.value).toBe(true);
      w.unmount();
    });

    it("asks for the 720px query by default", () => {
      const media = stubNarrow(true);
      withSetup(() => useNarrow()).unmount();
      expect(media.matchMedia).toHaveBeenCalledWith("(max-width: 720px)");
    });
  });

  describe("the window is wide", () => {
    it("is false", () => {
      stubNarrow(false);
      const w = withSetup(() => useNarrow());
      expect(w.result.value).toBe(false);
      w.unmount();
    });
  });

  describe("a custom query", () => {
    it("is passed to matchMedia as is", () => {
      const media = stubNarrow(true);
      withSetup(() => useNarrow("(max-width: 480px)")).unmount();
      expect(media.matchMedia).toHaveBeenCalledWith("(max-width: 480px)");
    });
  });

  describe("the window is resized", () => {
    it("a change event flips the ref both ways", () => {
      const media = stubNarrow(false);
      const w = withSetup(() => useNarrow());
      media.change(true);
      expect(w.result.value, "after shrinking").toBe(true);
      media.change(false);
      expect(w.result.value, "after growing").toBe(false);
      w.unmount();
    });
  });

  describe("the component is unmounted", () => {
    it("the change listener is removed", () => {
      const media = stubNarrow(false);
      const w = withSetup(() => useNarrow());
      expect(media.listenerCount(), "subscribed while mounted").toBe(1);
      w.unmount();
      expect(media.listenerCount(), "unsubscribed after unmount").toBe(0);
    });
  });

  describe("there is no matchMedia", () => {
    it("is false and nothing crashes", () => {
      vi.stubGlobal("matchMedia", undefined);
      const w = withSetup(() => useNarrow());
      expect(w.result.value).toBe(false);
      w.unmount();
    });
  });
});
