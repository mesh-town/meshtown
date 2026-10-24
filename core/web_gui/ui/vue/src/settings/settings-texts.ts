/* Texts of the "Settings" section: the page, the save bar, field captions, notes and action messages. */

import { NET_NAME } from "../common/words.ts";

// --- Page ---
export const SETTINGS_STORE_MISSING = "STORE_KEY not provided";
export const SETTINGS_RESTART_NOTE = "Fields marked \"after restart\" take effect after the node restarts: once you save your changes, a \"Restart Mesh Town\" button appears at the bottom.";
export const SETTINGS_LOADING = "Loading settings…";
export const SETTINGS_CONFLICT_TITLE = "The settings changed while you were editing";
export const SETTINGS_CONFLICT_HINT = "They were changed in another tab or by Mesh Town itself. Choose which changes to keep.";
export const SETTINGS_CONFLICT_RELOAD = "Reload (your changes will be lost)";
export const SETTINGS_CONFLICT_OVERWRITE = "Save my changes over them";
export const SETTINGS_FILE_LABEL = "Settings file";
export const SETTINGS_BROKEN_TITLE = "The settings file can't be read";
export const SETTINGS_BROKEN_HINT = "If you save the settings here, the file will be written anew and the old one will stay next to it under the name";
export const SETTINGS_BAD_FILE_NAME = "config.json.bad-<date>";
export const SETTINGS_FIELD_ERRORS_HINT = "Some of the settings in the file couldn't be read — defaults are used for them:";
export const SETTINGS_MAIN_TITLE = "General";
export const SETTINGS_NET_TITLE = "Network";
export const SETTINGS_LEAVE_TITLE = "Leave network";
export const SETTINGS_LEAVE_BTN = "Leave network…";
export const leaveHint = (net?: string | null): string =>
  "Disconnect this computer from " + (net ? "\"" + net + "\"" : "the network") + " for good. You'll need a new invite to come back.";
export const SETTINGS_STOP_PREFIX = "Stop ";
export const SETTINGS_EXPERT_TITLE = "Advanced";
export const SETTINGS_PAGE_TITLE = "Page";
export const SETTINGS_SAME_NETS_LINK = "change";

// --- Field row, token ---
export const SETTINGS_RESTART_BADGE = "after restart";
export const SETTINGS_TOKEN_TITLE = "Persistent node token";
export const SETTINGS_TOKEN_DESC = "Used by the tray icon and scripts. You don't need it to sign in: use your password or the link with a temporary token that Mesh Town prints at startup.";
export const SETTINGS_TOKEN_HIDE = "Hide";
export const SETTINGS_TOKEN_SHOW = "Show";
export const SETTINGS_TOKEN_REVOKE = "Revoke";

// --- Save bar ---
export const SETTINGS_DIRTY_HINT = "You have unsaved changes";
export const SETTINGS_SAVE = "Save";
export const SETTINGS_RESET = "Discard changes";
export const SETTINGS_RESTART_NODE = "Restart " + NET_NAME;

// --- Choice options ---
export const SETTINGS_EVERY_5_MIN = "every 5 minutes";
export const SETTINGS_EVERY_15_MIN = "every 15 minutes";
export const SETTINGS_EVERY_30_MIN = "every 30 minutes";
export const SETTINGS_EVERY_HOUR = "every hour";
export const SETTINGS_EVERY_6_HOURS = "every 6 hours";
export const SETTINGS_EVERY_MINUTE = "every minute";
export const SETTINGS_EVERY_10_MIN = "every 10 minutes";
export const SETTINGS_SOCKS_LOCALHOST = "This computer only";
export const SETTINGS_SOCKS_LOCALNET = "Local network devices (TV, phone)";
export const SETTINGS_WEB_LOCALHOST = "From this computer only";
export const SETTINGS_WEB_LOCALNET = "From this computer and the local network";

// --- Field captions ---
export const SETTINGS_DESC_SOCKS_ACCESS =
  "In local network mode the proxy has no password: any device on your network (and guest Wi-Fi, if it isn't separated) can use it. Takes effect after the node restarts.";
export const SETTINGS_TITLE_AUTOSTART = "Start at login";
export const SETTINGS_TITLE_SOCKS_ENABLED = "Browser proxy";
export const SETTINGS_DESC_SOCKS_ENABLED = "Turns on the browser proxy.";
export const SETTINGS_TITLE_SOCKS_PORT = "Proxy port (SOCKS5 and HTTP)";
export const SETTINGS_TITLE_SOCKS_ACCESS = "Who can use the proxy";
export const SETTINGS_TITLE_CHECKS_ENABLED = "Check sites from the list";
export const SETTINGS_TITLE_CHECKS_INTERVAL = "How often to check sites";
export const SETTINGS_TITLE_DOH_SERVER = "Server for checking site addresses";
export const SETTINGS_DESC_HOST_PORT = "An address like host:port.";
export const SETTINGS_TITLE_PROVIDER_DNS = "Your ISP's DNS (for comparison)";
export const SETTINGS_PLACEHOLDER_NOT_SET = "not set";
export const SETTINGS_DESC_PROVIDER_DNS = "Format: host:port. Leave empty to skip the comparison.";
export const SETTINGS_TITLE_PROBE_INTERVAL = "How often to check connection speed";
export const SETTINGS_DESC_PROBE_INTERVAL = "Checks whether a large chunk of data gets through " + NET_NAME + ": 16 KiB, 256 KiB, 1 MiB.";
export const SETTINGS_TITLE_LOG_RING = "Log size";
export const SETTINGS_DESC_LOG_RING = "How many lines are kept in the node's memory (from 200 to 20,000).";
export const SETTINGS_TITLE_FULL_TUNNEL = "All traffic through " + NET_NAME;
export const SETTINGS_TITLE_WEB_ENABLED = "Node web page";
export const SETTINGS_TITLE_WEB_PORT = "Web page port";
export const SETTINGS_TITLE_WEB_ACCESS = "Who can access the settings page";
export const SETTINGS_DESC_WEB_ACCESS = "This only concerns access to the node settings, not to the network or traffic. A change takes effect immediately; widening access to the local network when the page address is 127.0.0.1 takes effect after the node restarts.";

export function settingsFileValueText(value: string | number): string {
  return String(value) + " — set in the file";
}

// --- Notes on fields and the bar (settings-view) ---
export const SETTINGS_RESTART_STARTED = NET_NAME + " is restarting. The settings will take effect in a few seconds.";
export const SETTINGS_WRITABLE_REASON = "Can't save: the folder with the settings file is read-only";
export const SETTINGS_BROKEN_FILE = "The settings file is damaged";
export const SETTINGS_VALUE_ON = "on";
export const SETTINGS_VALUE_OFF = "off";
export const SETTINGS_CLI_NOTE_EDITABLE = "set at node startup — your edit will be saved to the file and take effect when you start the node without this command-line option";
export const SETTINGS_READONLY_WEB_ENABLED = "Can only be turned off in the settings file — otherwise the page would lock itself out";
export const SETTINGS_READONLY_DEFAULT = "Can only be changed in the settings file";
export const SETTINGS_SAVED_LATE = "Saved. Takes effect after " + NET_NAME + " restarts.";
export const SETTINGS_SAVED = "Saved";

export function settingsCliHeadText(inFile: string, effective: string): string {
  return "in file: " + inFile + " · currently in effect: " + effective + " — set at node startup";
}

export const SETTINGS_CLI_TAIL = ", your edit will be saved to the file and take effect when you start the node without this command-line option";

// --- Action messages (settings-actions) ---
export const SETTINGS_CONFIRM_ROTATE = "Old links to this page will stop working immediately; this tab will get the new token by itself. Continue?";
export const SETTINGS_FIELD_ERRORS_MESSAGE = "There are errors in the fields — see the notes below.";
export const SETTINGS_WRITE_FAILED = "Couldn't write the settings file";
export const SETTINGS_SAVING = "Saving…";
export const SETTINGS_FAIL_SAVE = "save";
export const SETTINGS_FAIL_RESTART = "restart the node";
export const SETTINGS_FAIL_STOP = "stop the node";
export const SETTINGS_FAIL_ROTATE = "change the token";
export const SETTINGS_TOKEN_ROTATED = "The new token has been issued and applied";

// --- Several networks ---
export const SETTINGS_PARTIAL = "Saved, but some networks did not accept it:";
export const SETTINGS_NAME_PER_NET = "Your name is set for each network: Members \u2192 the network's tab.";
export const SETTINGS_ALL_NETS = "Applies to all your networks.";
export const SETTINGS_STOP_ALL = "Stops all your networks. ";
export const SETTINGS_LEAVE_DELETE_MULTI = "To leave or delete a network, open Members \u2192 its tab.";
export const SETTINGS_LEAVE_MULTI = "To leave a network, open Members \u2192 its tab.";
export const SETTINGS_WEB_ACCESS_MULTI = "Which connections of this computer can open this page (Wi-Fi, cable)";
