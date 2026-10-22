/* A controllable `matchMedia` stub for tests of narrow-screen behavior: `stubNarrow(true)` makes every query match,
 * `change(next)` flips the result and notifies the "change" listeners, like a window resize would. */

import { vi } from "vitest";

export function stubNarrow(matches) {
  const listeners = new Set();
  const media = { matches, media: "" };
  const matchMedia = vi.fn((query) => {
    media.media = query;
    return {
      get matches() { return media.matches; },
      media: query,
      addEventListener: (type, cb) => { if (type === "change") listeners.add(cb); },
      removeEventListener: (type, cb) => { if (type === "change") listeners.delete(cb); },
      addListener() {},
      removeListener() {},
    };
  });
  vi.stubGlobal("matchMedia", matchMedia);
  return {
    matchMedia,
    listenerCount: () => listeners.size,
    change(next) {
      media.matches = next;
      for (const cb of [...listeners]) cb({ matches: next, media: media.media });
    },
  };
}
