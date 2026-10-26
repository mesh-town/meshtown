/* Texts of the Home logic (`.ts`): browser connection steps, the exit list, actions and the status panel,
 * invite, joining a network, join progress, the member ring. */

import { forDevice } from "../common/device.ts";
import { NET_NAME } from "../common/words.ts";

// Shared connection words (the exit list, the status panel)
export const HOME_LINK_ONLINE = "online";
export const HOME_LINK_OFFLINE = "offline";
export const HOME_EXIT_NOT_OFFERED = "not an exit";
export const HOME_LINK_DIRECT = "direct";
export const HOME_LINK_RELAY = "via relay server";
export const rttMsText = (ms: number): string => ms + " ms";
export const HOME_OS_PHONE = "Phone";

// browser-setup.ts
export const HOME_BROWSER_WARN = "This is a system-wide setting: other programs that use it will also go through " + NET_NAME + ".";
export const HOME_SAFARI_ONLY_MAC = "Safari exists only on macOS. Choose another browser here — Firefox is the most reliable.";
export const HOME_STEP_HOST = "Server address:";
export const HOME_STEP_SOCKS_HOST = "SOCKS host:";
export const HOME_STEP_PORT = "Port:";
export const HOME_FF_OPEN = "Open Firefox \"Settings\" → \"General\" → \"Network Settings\" → \"Settings…\".";
export const HOME_FF_MANUAL = "Choose \"Manual proxy configuration\".";
export const HOME_FF_SOCKS5 = "Select \"SOCKS v5\".";
export const HOME_FF_DNS = "Select \"Proxy DNS when using SOCKS v5\" and click \"OK\".";
export const HOME_MAC_OPEN = "Open \"System Settings\" → \"Network\" and choose your connection.";
export const HOME_MAC_DETAILS = "Click \"Details…\" and open \"Proxies\".";
export const HOME_MAC_SOCKS = "Turn on \"SOCKS proxy\".";
export const HOME_MAC_OK = "Click \"OK\".";
export const HOME_WIN_OPEN = "Open \"Settings\" → \"Network & Internet\" → \"Proxy\".";
export const HOME_WIN_MANUAL = "Under \"Manual proxy setup\" click \"Set up\" and turn on \"Use a proxy server\".";
export const HOME_WIN_FILL = "Enter:";
export const HOME_WIN_SAVE = "Click \"Save\". Chrome, Edge and other programs will go through " + NET_NAME +
  "; Firefox uses its own settings (the \"Firefox\" tab).";
export const HOME_LINUX_OPEN = "Open your system's network settings (in GNOME: \"Settings\" → \"Network\" → \"Network Proxy\").";
export const HOME_LINUX_MANUAL = "Choose manual configuration and fill in \"SOCKS\".";

// exit-list.ts, home-view.ts
export const HOME_WAIT_STARTING = NET_NAME + " is starting…";
export const HOME_WAIT_STOPPING = NET_NAME + " is stopping…";
export const HOME_DIRECT_PICK = "To open a blocked site, choose a member below.";
export const HOME_DIRECT_NONE = "No member offers their connection as an exit yet.";
export const HOME_EXIT_CHOSEN_FALLBACK = "the chosen member";
export const HOME_EXITS_OFFLINE = "The chosen members are offline";
export const offlinePeerText = (label: string): string => label + " is offline";

// home-actions.ts
export const HOME_SAVED = "Saved";
export const HOME_EMPTY_FIND = "Enter a site address, for example example.com";
export const HOME_FIND_ACTION = "check the site";
export const HOME_KIND_ADDR = "address";
export const HOME_KIND_PORT = "port";
export const HOME_KIND_LAN = "address for other devices";
export const clipboardFallbackText = (kindWord: string): string =>
  "Clipboard unavailable — select the " + kindWord + " in the step and copy it manually.";

// invite-actions.ts
export const HOME_INVITE_UNAVAILABLE = "The invite isn't ready yet — " + NET_NAME +
  " is still joining the network. Try again in a minute.";
export const HOME_INVITE_COPIED = "✓ Invite copied — paste it into a private message";
export const HOME_INVITE_MANUAL = "Clipboard unavailable — select the invite below and copy it manually.";
export const HOME_INVITE_STALE = "The network changed — click \"Invite a member\" again.";
export const HOME_REACH_ISSUER_ONLY = "Only this computer can let them in — don't turn it off until the new member joins.";
export const HOME_REACH_ISSUER_AND_ONE = "Better not to turn this computer off until the new member joins.";

// join-actions.ts
export const HOME_INVITE_EMPTY = "Paste the invite first.";
export const HOME_INVITE_INVALID = "This doesn't look like an invite. Copy the whole invite again and paste it here.";
export const HOME_LABEL_EMPTY = "Enter a name, for example \"family\".";
export const JOIN_STALLED = "Something went wrong: " + NET_NAME + " didn't respond after joining the network. What happened is on the \"Log\" tab.";
export const JOIN_STALLED_SILENT = "Something went wrong: " + NET_NAME + " didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started " + NET_NAME + " from.";
export const JOIN_WAITING = "Invite sent. Waiting for a member of this network to come online and let this computer in — this page will update.";
export const waitingStatusText = (netQuoted: string): string =>
  "Invite sent. Waiting for a member of" + netQuoted + forDevice(" to come online and let this computer in — this page will update.");
export const HOME_JOIN_ACTION = "join the network";
export const HOME_CREATE_FAILED = "Couldn't create the network — try again";
export const joinedStatusText = (netQuoted: string): string =>
  "Done: " + NET_NAME + " is restarting and joining the network" + netQuoted +
  ". The status will appear here in a few seconds.";

// join-progress.ts, join-wait.ts
export const SEARCH_SUB = "Waiting for the member who sent the invite (or another member listed in it) to come online. This usually takes up to a minute — just keep this page open.";
export const ENTER_SUB = NET_NAME + " is restarting and joining the network. This takes a few seconds.";
export const HOME_PROGRESS_SEARCHING = "Looking for the network…";
export const HOME_PROGRESS_ENTERING = "Joining the network…";
export const searchingMainText = (label: string): string => "Looking for the network" + label + "…";
export const enteringMainText = (label: string): string => "Joining the network" + label + "…";
export const inviteWaitingText = (netQuoted: string): string =>
  "Waiting for members of the network" + netQuoted + " to come online. As soon as one of the members from the invite is online, " + NET_NAME + " will join the network.";

// ring-text.ts
export const ringCenterMainText = (online: number, total: number): string => `${online} of ${total}`;
export const HOME_RING_ALONE = "only you";
export const HOME_RING_NO_NET = "no network";
export const ringViaGroupText = (n: number): string => `Exits: ${n}`;
export const ringClientGroupText = (n: number): string => `Opening sites through you: ${n}`;
export const ringOnlineGroupText = (n: number, alsoOnline = false): string => `${alsoOnline ? "Also online" : "Online"}: ${n}`;
export const ringOfflineGroupText = (n: number): string => `Offline: ${n}`;
export const ringViaLegendText = (label: string): string => `Your exit: ${label}`;
export const HOME_RING_SINGLE_VIA = "your exit, online";
export const HOME_RING_SINGLE_CLIENT = "opens sites through you";

// use-join.ts
export const HOME_TITLE_CREATE = "Create a new network";
export const HOME_TITLE_JOIN = "Join with an invite";
export const HOME_BUTTON_JOIN = "Join";
