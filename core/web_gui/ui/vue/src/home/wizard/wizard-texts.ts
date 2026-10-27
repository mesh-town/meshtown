/* Texts of the step-by-step setup: progress, navigation, questions, guide titles and per-browser steps. */

import { SCOPE_QUESTION } from "../../common/pac-texts.ts";
import { NET_NAME } from "../../common/words.ts";
import type { StepId, WizBrowser } from "./wizard-steps.ts";
import { HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_STEP_EXIT } from "../home-card-texts.ts";

export const WIZ_PROGRESS = (n: number, m: number): string => `Step ${n} of ${m}`;
export const WIZ_BACK = "Back";
export const WIZ_NEXT = "Next";
export const WIZ_SKIP = "Skip this step";
export const WIZ_FINISH = "Finish";

export const WIZ_Q_OS = "Which system are you setting up?";
export const WIZ_Q_BROWSER = "Which browser do you use?";
export const WIZ_QUESTIONS: Record<Exclude<StepId, "join" | "guide">, string> = {
  scope: SCOPE_QUESTION,
  os: WIZ_Q_OS,
  browser: WIZ_Q_BROWSER,
  exit: HOME_WIZARD_STEP_EXIT,
  connect: HOME_WIZARD_STEP_CONNECT,
};

export const WIZ_GUIDE_TITLE = (name: string): string => `Set up ${name}`;
export const BROWSER_NAMES: Record<WizBrowser, string> = { chrome: "Chrome", edge: "Edge", safari: "Safari", firefox: "Firefox", other: "Other" };

export const WIZ_COPY_ITEM = "Copy this address (PAC):";
export const WIZ_APP_LINK = "meshtown.apk";
export const WIZ_APP_ITEM = `On the phone, download the ${NET_NAME} app:`;
export const WIZ_EXIT_DIRECT_WARN = "Blocked sites won't open until you choose a member above.";
export const MANUAL_TOGGLE_HAND = "Set up by hand (if the steps above didn't work) ▸";
export const MANUAL_TOGGLE_HAND_OPEN = "Set up by hand (if the steps above didn't work) ▾";

export const ALL_APPS_STEPS: Record<WizBrowser, string[]> = {
  chrome: [
    "Chrome uses the system setting from the steps above — there is nothing to change in Chrome itself.",
    "If Chrome was open, quit it completely and open it again.",
  ],
  edge: [
    "Edge uses the system setting from the steps above — there is nothing to change in Edge itself.",
    "If Edge was open, close all its windows and open it again.",
  ],
  safari: [
    "Safari always uses the system setting from the steps above — there is nothing to change in Safari.",
    "If Safari was open, quit it (\"Safari\" → \"Quit Safari\") and open it again.",
  ],
  firefox: [
    "Open Firefox \"Settings\" → \"General\", scroll down to \"Network Settings\" and click \"Settings…\".",
    "Make sure \"Use system proxy settings\" is selected — it is the default.",
    "Click \"OK\".",
  ],
  other: [`Opera, Brave, Vivaldi, Arc and Yandex Browser use the system setting, like Chrome. In Opera, turn off its built-in VPN — otherwise sites bypass ${NET_NAME}.`],
};
// Chrome has no Quit command on Windows and Linux: all windows are closed from the menu.
export const ALL_APPS_CHROME_CLOSE = "If Chrome was open, close all its windows (menu ⋮ → Exit) and open it again.";
export const ALL_APPS_CHROME_LINUX = "On Linux, Chrome follows the GNOME or KDE proxy setting.";

export const WIZ_ONE_BROWSER = (name: string): string =>
  `${name} has no proxy setting of its own — it always uses the system one. To use ${NET_NAME} in ${name}, set it up for all apps on this computer.`;
export const WIZ_ONE_BROWSER_OTHER = 
  `This browser has no proxy setting of its own — it always uses the system one. To use ${NET_NAME} in this browser, set it up for all apps on this computer.`;
export const WIZ_TO_SYSTEM = "Set up for all apps";
