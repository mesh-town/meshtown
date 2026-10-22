/* Texts of the "Connect the browser" card and of stopping the node: fixed texts.
 * The product name comes from `NET_NAME`; consumers (the card, the "Settings" section, the confirmation dialog)
 * import the constants from here. */

import { NET_NAME } from "./words.ts";

export const PAC_TITLE = "Connect the browser to " + NET_NAME;
export const PAC_INTRO = `Enter this address once in your computer's proxy settings. After that, the browser opens sites through ${NET_NAME} by itself whenever you choose an exit. No password needed.`;
export const PAC_ADDR_LABEL = "Auto-config address (PAC):";
export const PAC_WHY = `This is the address the browser uses to find out which sites to open through ${NET_NAME}.`;
export const PAC_COPY_FALLBACK = "Clipboard unavailable — select the address and copy it manually.";
export const PAC_NO_EXIT_NOTE = `While "Exit via…" is set to "Directly, without an exit", sites open directly. That's expected.`;

export const OS_LABEL = "Your system:";
export const WINDOWS_SOON = "We are still checking the steps for Windows — for now use \"Not working? Set up manually\" below.";
export const PHONE_SOON = "iPhone and Android are coming soon. For now, connect the browser on your computer.";

export const MANUAL_TOGGLE = "Not working? Set up manually ▸";
export const MANUAL_TOGGLE_OPEN = "Not working? Set up manually ▾";
export const MANUAL_INTRO_BASE = "For programs that don't support automatic setup, enter the proxy by hand — type SOCKS5, with the address and port below.";
export const MANUAL_TAIL_MAC = "First turn off \"Automatic Proxy Configuration\" if you turned it on following the steps above.";
export const MANUAL_TAIL_LINUX = "If you turned on auto-config following the steps above, first switch it to manual mode.";
export const MANUAL_TAIL_WINDOWS = "If you have already turned on proxy auto-config, turn it off first.";
export const PAC_COPY_ARIA = "Copy address of the PAC file";
export const HOST_COPY_ARIA = "Copy address of the proxy";
export const MANUAL_BROWSER_HINT = "Firefox is configured separately — choose it below. The steps are the same for Chrome, Edge and Safari.";
export const MANUAL_BROWSER_HINT_NO_SAFARI = "Firefox is configured separately — choose it below. The steps are the same for Chrome and Edge.";
export const WIZARD_AGAIN = "Open step-by-step setup";

export const STOPPED_NOTE = `While ${NET_NAME} is stopped, sites in the browser don't open (local addresses do). After it starts, sites come back in about 15 seconds. To start it again, use the ${NET_NAME} icon near the clock → "Start ${NET_NAME}".`;
export const CONFIRM_STOP_TEXT = `Stop ${NET_NAME}? While it is stopped, sites in the browser don't open (local addresses do); after it starts, they come back in about 15 seconds.`;

export const MAC_STEPS: string[] = [
  "Click \"Copy address\" above.",
  "Open \"System Settings\" → \"Network\" and choose your connection (Wi‑Fi or Ethernet).",
  "Click \"Details…\" and open \"Proxies\".",
  "If \"SOCKS proxy\" is on (for example, from earlier " + NET_NAME + " instructions), turn it off.",
  "Turn on \"Automatic Proxy Configuration\" (in some versions — \"Auto Proxy Configuration\") and paste the address into the \"URL\" field.",
  "Click \"OK\".",
];
export const MAC_SMALL: string[] = [
  "The setting applies to one connection: if you switch between Wi‑Fi and Ethernet, repeat it for the other one.",
  "On macOS 12 and earlier: \"Network\" → \"Advanced…\" → \"Proxies\".",
];

export const LINUX_STEPS: string[] = [
  "Click \"Copy address\" above.",
  "Open \"Settings\" → \"Network\" → \"Proxy\".",
  "Turn on \"Network Proxy\", choose \"Automatic\" in the \"Configuration\" list and paste the address into the \"Configuration URL\" field.",
  "Click \"Save\" in the window header. GNOME 44 has no such button — the address is applied immediately.",
];
export const LINUX_SMALL: string[] = [
  "On GNOME 43 and earlier: \"Settings\" → \"Network\" → the \"Network Proxy\" row → the gear; choose \"Automatic\" and paste the address into the \"Configuration URL\" field. There is no \"Save\" button — the address is applied immediately.",
  "On KDE: \"System Settings\" → \"Network & Connectivity\" → \"Wi‑Fi & Internet\" (in Plasma 5 — \"Network Settings\") → \"Proxy\" → \"Use proxy configuration URL\".",
];
