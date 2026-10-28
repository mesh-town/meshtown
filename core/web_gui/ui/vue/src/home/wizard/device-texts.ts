// Texts for connecting a phone or a computer through a router, and for an iPhone that has no router.

import type { OsId } from "../browser-setup.ts";
import { NET_NAME } from "../../common/words.ts";
import { SETTINGS_SOCKS_LOCALNET, SETTINGS_TITLE_SOCKS_ACCESS } from "../../settings/settings-texts.ts";

export const DEVICE_LABELS: Record<OsId, string> = {
  mac: "a Mac",
  windows: "a Windows computer",
  linux: "a Linux computer",
  android: "an Android phone",
  ios: "an iPhone or iPad",
};

export const ROUTER_GUIDE_TITLE = (label: string): string => `Connect ${label} to the router`;

export const SITE_DEVICES_URL = "https://mesh-town.github.io/install/openwrt/#devices";
export const IOS_ROUTER_LINK = "Router setup guide";

export const IOS_WIFI_STEPS: string[] = [
  "On the iPhone or iPad, open \"Settings\" → \"Wi‑Fi\" and tap ⓘ next to the network you are connected to.",
  "At the bottom, tap \"Configure Proxy\" and choose \"Automatic\".",
  "Paste the address into the \"URL\" field and tap \"Save\".",
];

export const ANDROID_WIFI_STEPS: string[] = [
  "On the phone, open the Wi‑Fi settings and tap the gear next to the network you are connected to.",
  "Tap \"Edit\" or \"Advanced\", set \"Proxy\" to \"Proxy Auto-Config\" and paste the address into \"PAC URL\".",
  "Tap \"Save\".",
];

export const WIFI_ONLY_SMALL = "The setting belongs to this Wi‑Fi network: on mobile data and other networks the phone opens sites directly.";
export const ANDROID_APP_SMALL = `To use ${NET_NAME} away from home too, install the ${NET_NAME} app instead: it joins the network on its own.`;

export const IOS_VIA_COMPUTER = "This computer shares its proxy with the local network. Connect the iPhone or iPad to the same Wi‑Fi and enter this address in its Wi‑Fi settings; keep this computer on while you use it.";
export const IOS_ROUTER_ITEM = `Install ${NET_NAME} on your router (OpenWrt or Keenetic) and connect the iPhone to it:`;
export const IOS_SHARE_ITEM = `Or share this computer's proxy: in "Settings", set "${SETTINGS_TITLE_SOCKS_ACCESS}" to "${SETTINGS_SOCKS_LOCALNET}" and restart ${NET_NAME}. This step then shows the address for the iPhone.`;
