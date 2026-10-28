/* Texts of the step-by-step setup: progress, navigation, questions, guide titles and per-browser steps. */

import { SCOPE_QUESTION } from "../../common/pac-texts.ts";
import { NET_NAME } from "../../common/words.ts";
import type { StepId, WizBrowser } from "./wizard-steps.ts";
import { HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_STEP_EXIT } from "../home-card-texts.ts";

export const WIZ_PROGRESS = (n: number, m: number): string => `Step ${n} of ${m}`;
export const WIZ_BACK = "Back";
export const WIZ_STRIP_LABEL = "Setup steps";
export const WIZ_STEP_NAMES: Record<StepId, string> = {
  join: "Join", scope: "Apps", os: "System", browser: "Browser", guide: "Set up", exit: "Exit", connect: "Connect",
};
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

export const WIZ_ALL_APPS_TITLE = (os: string): string => `Set up all apps on ${os}`;
export const ALL_APPS_BROWSERS_TITLE = "Browsers";
export const ALL_APPS_BROWSER_STEPS: string[] = [
  "Chrome, Edge, Safari, Opera, Brave and most other browsers use this system setting — there is nothing to change in them. If a browser was open, close it completely and open it again.",
  "Firefox: open \"Settings\" → \"General\" → \"Network Settings\" → \"Settings…\" and make sure \"Use system proxy settings\" is selected (it is the default).",
  `Opera: turn off its built-in VPN — otherwise sites bypass ${NET_NAME}.`,
];
export const ALL_APPS_LINUX_SMALL = "On Linux, Chrome and other Chromium-based browsers follow the GNOME or KDE proxy setting.";

export const WIZ_ONE_BROWSER = (name: string): string =>
  `${name} has no proxy setting of its own — it always uses the system one. To use ${NET_NAME} in ${name}, set it up for all apps on this computer.`;
export const WIZ_ONE_BROWSER_OTHER =
  "Most browsers (Opera, Brave, Vivaldi, Arc, Yandex Browser) have no proxy setting of their own and use the system one — set up all apps on this computer. If your browser has its own proxy setting (Firefox-based browsers do), paste the PAC address into it instead.";
export const WIZ_TO_SYSTEM = "Set up for all apps";
