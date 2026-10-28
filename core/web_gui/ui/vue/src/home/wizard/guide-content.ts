// What the guide screen shows for a (scope, system, browser) answer. Pure functions, no framework imports.

import { ANDROID_INTRO, ANDROID_STEPS, FIREFOX_STEPS, IOS_NO_APP } from "../../common/pac-texts.ts";
import type { BrowserId, OsId } from "../browser-setup.ts";
import { pacBlock, PAC_OS_LIST } from "../pac-setup.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { UPDATE_ANDROID_DOWNLOAD_URL } from "../update-texts.ts";
import { isPhoneOs } from "./wizard-steps.ts";
import type { WizBrowser } from "./wizard-steps.ts";
import { ALL_APPS_BROWSER_STEPS, ALL_APPS_BROWSERS_TITLE, ALL_APPS_LINUX_SMALL, BROWSER_NAMES, WIZ_ALL_APPS_TITLE, WIZ_APP_ITEM, WIZ_GUIDE_TITLE, WIZ_ONE_BROWSER, WIZ_ONE_BROWSER_OTHER } from "./wizard-texts.ts";

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

function browserGuide(browser: WizBrowser, title: string): Guide {
  if (browser === "other") return { ...BLANK, title, copy: true, intro: WIZ_ONE_BROWSER_OTHER, toSystem: true };
  if (browser !== "firefox") return { ...BLANK, title, intro: WIZ_ONE_BROWSER(BROWSER_NAMES[browser]), toSystem: true };
  return { ...BLANK, title, copy: true, steps: FIREFOX_STEPS.map((text) => ({ text })), more: true };
}

function systemGuide(os: OsId): Guide {
  const block = pacBlock(os);
  return {
    ...BLANK, title: WIZ_ALL_APPS_TITLE(osLabel(os)), copy: true, more: true,
    steps: block.steps.map((text) => ({ text })), small: block.small,
    browserTitle: ALL_APPS_BROWSERS_TITLE, browserSteps: ALL_APPS_BROWSER_STEPS,
    browserSmall: os === "linux" ? [ALL_APPS_LINUX_SMALL] : [],
  };
}

export function guideFor(scope: ProxyScope, os: OsId, browser: WizBrowser): Guide {
  if (isPhoneOs(os)) return phoneGuide(os);
  if (scope !== "browser") return systemGuide(os);
  return browserGuide(browser, WIZ_GUIDE_TITLE(browser === "other" ? "your browser" : BROWSER_NAMES[browser]));
}
