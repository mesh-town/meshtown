/* Browser connection steps by the PAC address: the list of systems for the switcher and the text block
 * (steps, small lines, placeholder) for the chosen system. Pure functions. */

import { HOST_COPY_ARIA, MANUAL_BROWSER_HINT, MANUAL_BROWSER_HINT_NO_SAFARI, LINUX_SMALL, LINUX_STEPS, MANUAL_INTRO_BASE, MANUAL_TAIL_LINUX, MANUAL_TAIL_MAC, MANUAL_TAIL_WINDOWS, MAC_SMALL, MAC_STEPS, PAC_COPY_ARIA, ANDROID_INTRO, ANDROID_STEPS, IOS_NO_APP, WINDOWS_SMALL, WINDOWS_STEPS } from "../common/pac-texts.ts";
import type { BrowserId, OsId, StepValue } from "./browser-setup.ts";
import { HOME_COPIED } from "./home-card-texts.ts";

export interface PacBlock {
  steps: string[];
  small: string[];
  note: string;
}

export const PAC_OS_LIST: [OsId, string][] = [["mac", "macOS"], ["windows", "Windows"], ["linux", "Linux"], ["android", "Android"], ["ios", "iOS"]];

const BLOCKS: Record<OsId, PacBlock> = {
  mac: { steps: MAC_STEPS, small: MAC_SMALL, note: "" },
  linux: { steps: LINUX_STEPS, small: LINUX_SMALL, note: "" },
  windows: { steps: WINDOWS_STEPS, small: WINDOWS_SMALL, note: "" },
  android: { steps: ANDROID_STEPS, small: [], note: ANDROID_INTRO },
  ios: { steps: [], small: [], note: IOS_NO_APP },
};

const MANUAL_INTROS: Record<OsId, string> = {
  mac: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_MAC,
  linux: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_LINUX,
  windows: MANUAL_INTRO_BASE + " " + MANUAL_TAIL_WINDOWS,
  android: MANUAL_INTRO_BASE,
  ios: MANUAL_INTRO_BASE,
};

/** The manual block exists on all systems except Android and iOS. */
export function manualAvailable(os: OsId): boolean {
  return os !== "android" && os !== "ios";
}

export function manualIntro(os: OsId): string {
  return MANUAL_INTROS[os];
}

const BROWSERS: [BrowserId, string][] = [["chrome", "Chrome"], ["edge", "Edge"], ["safari", "Safari"], ["firefox", "Firefox"]];

/** The "Your browser" switcher: Safari exists only on macOS. */
export function browserList(os: OsId): [BrowserId, string][] {
  return BROWSERS.filter(([id]) => id !== "safari" || os === "mac");
}

const BROWSER_HINTS: Record<OsId, string> = { mac: MANUAL_BROWSER_HINT, windows: MANUAL_BROWSER_HINT_NO_SAFARI, linux: MANUAL_BROWSER_HINT_NO_SAFARI, android: MANUAL_BROWSER_HINT_NO_SAFARI, ios: MANUAL_BROWSER_HINT_NO_SAFARI };

export function manualBrowserHint(os: OsId): string {
  return BROWSER_HINTS[os];
}

const COPY_ARIA: Partial<Record<StepValue, string>> = { pac: PAC_COPY_ARIA, host: HOST_COPY_ARIA };

/** The copy button caption for screen readers; undefined for port/lan (the name comes from the visible text). */
export function copyAriaLabel(value: StepValue, copied: unknown): string | undefined {
  return copied === value && value in COPY_ARIA ? HOME_COPIED : COPY_ARIA[value];
}

export function pacBlock(os: OsId): PacBlock {
  return BLOCKS[os];
}
