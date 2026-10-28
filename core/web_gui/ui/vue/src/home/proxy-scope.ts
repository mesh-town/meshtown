// The wizard's answers kept in the page storage next to the setup mark: where the proxy is meant to work (the whole
// system or one browser), and the system and the browser the user clicked. Garbage and a missing storage read as
// "not chosen"; storage errors are swallowed.

import type { OsId } from "./browser-setup.ts";
import type { WizardAnswers } from "./wizard/wizard-machine.ts";
import type { WizBrowser } from "./wizard/wizard-steps.ts";

export type ProxyScope = "system" | "browser";

interface ScopeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const SCOPE_KEY = "meshtown.proxyScope";
const OS_KEY = "meshtown.proxyOs";
const BROWSER_KEY = "meshtown.proxyBrowser";
const OS_IDS: string[] = ["mac", "windows", "linux", "android", "ios"];
const BROWSER_IDS: string[] = ["chrome", "edge", "safari", "firefox", "other"];

function readKnown(storage: ScopeStorage | null, key: string, known: string[]): string | null {
  try {
    const raw = storage!.getItem(key);
    return raw !== null && known.includes(raw) ? raw : null;
  } catch (err) {
    return null;
  }
}

function write(storage: ScopeStorage | null, key: string, value: string): void {
  try {
    storage!.setItem(key, value);
  } catch (err) {
    /* storage is unavailable — the choice is asked again next time */
  }
}

// Anything but the two known values (including no storage) is "not chosen yet".
export function readProxyScope(storage: ScopeStorage | null): ProxyScope | null {
  return readKnown(storage, SCOPE_KEY, ["system", "browser"]) as ProxyScope | null;
}

export function writeProxyScope(scope: ProxyScope, storage: ScopeStorage | null): void {
  write(storage, SCOPE_KEY, scope);
}

export function saveOs(os: OsId, storage: ScopeStorage | null): void {
  write(storage, OS_KEY, os);
}

export function saveBrowser(browser: WizBrowser, storage: ScopeStorage | null): void {
  write(storage, BROWSER_KEY, browser);
}

/** The answers remembered from earlier visits; what was only detected is never written, so it is `null` here. */
export function loadAnswers(storage: ScopeStorage | null): WizardAnswers {
  return {
    scope: readProxyScope(storage),
    os: readKnown(storage, OS_KEY, OS_IDS) as OsId | null,
    pick: readKnown(storage, BROWSER_KEY, BROWSER_IDS) as WizBrowser | null,
  };
}
