/* Texts of the networks block, fixed wording. The words localhost, WAN, LAN and "interface" do not appear. */

import { NET_NAME } from "../common/words.ts";

export const NETS_TITLE = "Which local networks can connect";
export const NETS_SUB = "Applies to both the proxy and the settings page.";
export const NETS_TAG = "after restart";
export const AUTO_NOTE = "The networks are currently chosen automatically.";
export const AUTO_BTN = "Return to automatic choice";
export const EMPTY_HINT = "Select at least one network.";
export const SAVED_NOTE = "Saved. Takes effect after " + NET_NAME + " restarts.";
export const ROUTER_RESTART_NOTE = "The page will disappear for a few seconds and reopen on its own.";

export const WARN_PROVIDER =
  "Then any device on the network the router is connected to will be able to use the proxy without a password: for example, on the main home router's network or your ISP's. Select only if that is also your network.";
export const HERE_NOTE = "This page is open through this network — you can't deselect it here. Open the page from another network or from this computer itself.";

export const PAC_INTRO_ROUTER =
  "Enter this address once in the proxy settings of a phone or computer connected to the router. After that, the browser opens sites through Mesh Town by itself whenever you choose an exit. No password needed.";
export const LAN_HINT_MULTI = "Use the address of the network the device is connected to.";
export const ROUTER_ONLY_TEXT = "The proxy is open only to the router itself — phones and computers at home won't be able to use it.";

export const DESC_SOCKS_ACCESS_NETS =
  "In local network mode the proxy has no password: any device on the networks selected below can use it. Takes effect after " + NET_NAME + " restarts.";

// Network names (`iface-title.ts`), summary lines and service messages.
export const NETS_IFACE_PROVIDER_HINT = "The router reaches the internet through this network";
export const NETS_IFACE_HOME_HINT = "Phones and computers connected to the router";
export const NETS_IFACE_VM_TITLE = "Network of virtual machines and containers";
export const NETS_IFACE_PROVIDER_TITLE = "ISP network";
export const NETS_IFACE_HOME_TITLE = "Home network";
export const NETS_IFACE_GUEST_TITLE = "Guest network";
export const NETS_IFACE_VPN_TITLE = "VPN connection";

export function netsIfaceOtherTitleText(name: string): string {
  return "Other network \"" + name + "\"";
}

export const NETS_LEGACY_LAN_HINT = "Choose an address from the same network as the device: usually the first three numbers match.";

export function netsSameAsProxyText(titles: string[]): string {
  return "From the same networks as the proxy: " + titles.join(", ") + ".";
}

export function netsClosedForText(titles: string[]): string {
  return " Closed for: " + titles.join(", ") + ".";
}

export const NETS_PROVIDER_CLOSED_AUTO = " The ISP network is closed automatically.";
export const NETS_SUM_OPEN_FOR = "The proxy is open for:";
export const NETS_NO_STORE = "STORE_KEY not provided";
