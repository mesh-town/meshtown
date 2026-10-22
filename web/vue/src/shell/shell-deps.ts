/* The shell's default dependencies: network, timers, address and page storage. A shared set for
 * `useShell`, `adoptUrlToken` and `main.ts`; the poller needs `api`, timers and `isHidden`. */

import { api as coreApi } from "../core/api.ts";
import { getToken } from "../core/token.ts";
import type { ApiFn } from "../core/api.ts";

/** The shell's dependencies: network, timers, address and page storage (replaced in tests). */
export interface ShellDeps {
  api: ApiFn;
  getToken: () => string;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
  isHidden: () => boolean;
  replaceUrl: (url: string) => void;
  scrollToTop: () => void;
  readonly href: string;
  readonly hash: string;
  readonly storage: Pick<Storage, "getItem" | "setItem"> | undefined;
  readonly doc: EventTarget | undefined;
  readonly win: EventTarget | undefined;
}

// Globals are taken at call time, not at import: tests run in node without a DOM.
export const defaultShellDeps: ShellDeps = {
  api: coreApi as ApiFn,
  getToken: () => getToken(),
  setTimer: (fn: () => void, ms: number) => setTimeout(fn, ms),
  clearTimer: (id: unknown) => clearTimeout(id as ReturnType<typeof setTimeout> | undefined),
  isHidden: () => typeof document !== "undefined" && document.hidden,
  replaceUrl: (url) => history.replaceState(null, "", url),
  scrollToTop: () => scrollTo(0, 0),
  get href() {
    return typeof location === "undefined" ? "" : location.href;
  },
  get hash() {
    return typeof location === "undefined" ? "" : location.hash;
  },
  get storage() {
    return typeof sessionStorage === "undefined" ? undefined : sessionStorage;
  },
  get doc() {
    return typeof document === "undefined" ? undefined : document;
  },
  get win() {
    return typeof window === "undefined" ? undefined : window;
  },
};
