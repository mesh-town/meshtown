// What the guide screen shows for a (scope, system, browser) answer. Pure functions, no framework imports.

import { ANDROID_INTRO, ANDROID_STEPS, FIREFOX_STEPS, IOS_NO_APP } from "../../common/pac-texts.ts";
import type { BrowserId, OsId } from "../browser-setup.ts";
import { pacBlock, PAC_OS_LIST } from "../pac-setup.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { UPDATE_ANDROID_DOWNLOAD_URL } from "../update-texts.ts";
import { isPhoneOs } from "./wizard-steps.ts";
import type { WizBrowser } from "./wizard-steps.ts";
import { ALL_APPS_CHROME_CLOSE, ALL_APPS_CHROME_LINUX, ALL_APPS_STEPS, BROWSER_NAMES, WIZ_APP_ITEM, WIZ_GUIDE_TITLE, WIZ_ONE_BROWSER, WIZ_ONE_BROWSER_OTHER } from "./wizard-texts.ts";

export interface GuideStep {
  text: string;
  href?: string;
}

export interface Guide {
  title: string;
  copy: boolean;
  intro: string;
  steps: GuideStep[];
  small: string[];
  browserTitle: string;
  browserSteps: string[];
  browserSmall: string[];
  toSystem: boolean;
  more: boolean;
}

const BLANK: Guide = { title: "", copy: false, intro: "", steps: [], small: [], browserTitle: "", browserSteps: [], browserSmall: [], toSystem: false, more: false };

const TILES: Record<OsId, BrowserId[]> = {
  mac: ["safari", "chrome", "firefox", "edge"],
  windows: ["chrome", "edge", "firefox"],
  linux: ["firefox", "chrome"],
  android: [],
  ios: [],
};

export function browserTiles(os: OsId): [WizBrowser, string][] {
  if (isPhoneOs(os)) return [];
  const ids: WizBrowser[] = [...TILES[os], "other"];
  return ids.map((id): [WizBrowser, string] => [id, BROWSER_NAMES[id]]);
}

export function defaultBrowser(os: OsId, detected: BrowserId): WizBrowser {
  return TILES[os].includes(detected) ? detected : "other";
}

export function manualBrowser(b: WizBrowser): BrowserId {
  return b === "other" ? "chrome" : b;
}

function osLabel(os: OsId): string {
  return PAC_OS_LIST.find(([id]) => id === os)![1];
}

function phoneGuide(os: OsId): Guide {
  const title = WIZ_GUIDE_TITLE(osLabel(os));
  if (os === "ios") return { ...BLANK, title, intro: IOS_NO_APP };
  const steps = [{ text: WIZ_APP_ITEM, href: UPDATE_ANDROID_DOWNLOAD_URL }, ...ANDROID_STEPS.map((text) => ({ text }))];
  return { ...BLANK, title, intro: ANDROID_INTRO, steps };
}

function oneBrowserIntro(browser: WizBrowser): string {
  return browser === "other" ? WIZ_ONE_BROWSER_OTHER : WIZ_ONE_BROWSER(BROWSER_NAMES[browser]);
}

function browserGuide(browser: WizBrowser, title: string): Guide {
  if (browser !== "firefox") return { ...BLANK, title, intro: oneBrowserIntro(browser), toSystem: true };
  return { ...BLANK, title, copy: true, steps: FIREFOX_STEPS.map((text) => ({ text })), more: true };
}

function browserSteps(os: OsId, browser: WizBrowser): string[] {
  const steps = ALL_APPS_STEPS[browser];
  if (browser !== "chrome" || os === "mac") return steps;
  return [...steps.slice(0, -1), ALL_APPS_CHROME_CLOSE];
}

function systemGuide(os: OsId, browser: WizBrowser, title: string): Guide {
  const block = pacBlock(os);
  const linuxChrome = browser === "chrome" && os === "linux" ? [ALL_APPS_CHROME_LINUX] : [];
  return {
    ...BLANK, title, copy: true, more: true,
    steps: block.steps.map((text) => ({ text })), small: block.small,
    browserTitle: BROWSER_NAMES[browser], browserSteps: browserSteps(os, browser), browserSmall: linuxChrome,
  };
}

export function guideFor(scope: ProxyScope, os: OsId, browser: WizBrowser): Guide {
  if (isPhoneOs(os)) return phoneGuide(os);
  const title = WIZ_GUIDE_TITLE(browser === "other" ? "your browser" : BROWSER_NAMES[browser]);
  return scope === "browser" ? browserGuide(browser, title) : systemGuide(os, browser, title);
}
