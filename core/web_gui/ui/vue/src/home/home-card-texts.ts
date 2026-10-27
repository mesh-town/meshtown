/* Texts of the Home cards (`.vue` templates): browser connection, exit choice, invite, joining a network,
 * computer name, the ports question, the "Three steps" setup, the status panel. */

import { forDevice } from "../common/device.ts";
import { EXIT_DOWN_TAIL, NET_NAME } from "../common/words.ts";

// HomeSection
export const HOME_LOADING = "Loading…";

// BrowserCard
export const HOME_COPY_ADDR = "Copy address";
export const HOME_COPY_PORT = "Copy port";
export const HOME_COPY_SHORT = "Copy";
export const HOME_COPIED = "Copied";
export const HOME_LAN_LEAD_SINGLE = "For other devices on the network: address";
export const HOME_LAN_LEAD_HOME = "For a phone and TV on the home network: address";
export const HOME_LAN_LEAD_MULTI = "For other devices:";
export const HOME_LAN_TYPE = ", type HTTP or SOCKS5";
export const HOME_PROXY_OFF = "The browser proxy is off — turn it on in";
export const HOME_PROXY_OFF_LINK = "\"Settings\"";
export const HOME_OS_GROUP_LABEL = "Your system";
export const HOME_BROWSER_GROUP_LABEL = "Your browser";

// ExitCard
export const HOME_EXIT_TITLE = "Exit via…";
export const HOME_EXIT_EXPLAIN = "An exit is a member whose internet you use to open sites: sites see their address, not yours.";
export const HOME_EXIT_DIRECT = "Directly, without an exit";
export const HOME_EXIT_SEARCHING = "Looking for other members… The list fills in by itself as they are found.";
export const HOME_EXIT_NONE = "No other member offers their connection as an exit yet, so sites open directly.";
export const HOME_EXIT_LIST_HINT = "Sites go through the one member you choose. If that member goes offline, " + EXIT_DOWN_TAIL + " until you choose another.";
export const HOME_FIND_LEAD = "Not sure which member to use? Check a site:";
export const HOME_FIND_PLACEHOLDER = "for example, example.com";
export const HOME_FIND_ARIA = "Site to check";
export const HOME_FIND_BTN = "Check";
export const HOME_FIND_NO_NET = "Join a " + NET_NAME + " network to check sites through other members.";
export const findCheckingText = (domain: string): string => "Checking " + domain + " with other members…";
export const findEmptyText = (domain: string): string => domain + " doesn't open for any other member.";
export const findOkText = (domain: string): string => domain + " opens via:";

// InviteCard
export const HOME_INVITE_TITLE = "Network invite";
export const HOME_INVITE_BTN = "Invite a member";
export const HOME_INVITE_HINT = "The invite is copied when you click the button. It works once, for 24 hours — send it to one person in a private message.";
export const HOME_INVITE_FOR_LABEL = "For whom (optional, members will see it)";
export const HOME_INVITE_VALUE_ARIA = "Invite to send";

// JoinCard
export const HOME_JOIN_FIELD_LABEL = "Invite you received";
export const HOME_JOIN_PLACEHOLDER = "Paste the invite here";
export const HOME_JOIN_ERROR_ARIA = "Invite error";
export const HOME_JOIN_KEY_HINT = (net: string): string => `This is a network key, not an invite. Joining with it replaces "${net}"` + forDevice(" on this computer.");
export const HOME_JOIN_PASTE_HINT = "Paste the entire invite. If a word has a typo, " + NET_NAME + " will highlight it.";
export const HOME_ASK_MEMBER_HINT = (net: string): string => `Ask any member of "${net}" for a new invite and paste it here.`;
export const HOME_ASK_ADMIN_HINT = (net: string): string => `Ask an admin of "${net}" for a new invite and paste it here.`;
export const HOME_NO_RETURN_HINT = (net: string): string => forDevice("This computer can't rejoin ") + `"${net}". To keep using Mesh Town, paste an invite to another network.`;
export const HOME_JOIN_BUSY = "Joining…";
export const HOME_CREATE_TOGGLE = "No invite? Create a new " + NET_NAME + " network";
export const HOME_CREATE_LABEL = "Network name (members will see it in the invite)";
export const HOME_CREATE_PLACEHOLDER = "for example, family";
export const HOME_CREATE_ERROR_ARIA = "Network name error";
export const HOME_CREATE_BUSY = "Creating…";
export const HOME_CREATE_BTN = "Create network";

// NodeNameField
export const HOME_NAME_LABEL = "Name of this computer";
export const HOME_NAME_LABEL_PHONE = "Name of this phone";
export const HOME_NAME_PLACEHOLDER = "for example, Home laptop";
export const HOME_NAME_HINT = "Others see you by this name; leave it empty for a random one.";

// SetupWizard
export const HOME_WIZARD_HEAD_JOIN = "Join a " + NET_NAME + " network";
export const HOME_WIZARD_NOT_JOINED = "You haven't joined a " + NET_NAME + " network yet";
export const HOME_WIZARD_INVITE_HINT = "Got an invite? Join that network. No invite? Create your own network and invite people you trust.";
export const HOME_WIZARD_JOIN_BTN = "Join with an invite";
export const HOME_WIZARD_CREATE_BTN = "Create a new network";
export const HOME_WIZARD_TITLE = "Three steps to get started";
export const HOME_WIZARD_JOINED = "✓ You joined the network";
export const HOME_WIZARD_JOINED_NOT_RUNNING = "The network is saved, but " + NET_NAME + " isn't running yet";
export const HOME_WIZARD_STEP_BROWSER = "Set up your browser";
export const HOME_WIZARD_BROWSER_DONE = "Done, my browser is set up";
export const HOME_WIZARD_STEP_EXIT = "Choose an exit";
export const HOME_WIZARD_DONE = "Done";
export const HOME_WIZARD_STEP_CONNECT = "Tap Connect at the bottom of the screen";
export const HOME_WIZARD_STRICT_Q = "If " + NET_NAME + " isn't running:";
export const HOME_WIZARD_STRICT_OFF = "Open sites directly (recommended)";
export const HOME_WIZARD_STRICT_ON = "Don't open sites — for services that refuse sign-ins from another country";
export const HOME_PHONE_TITLE = "This phone";
export const HOME_NAME_SAVE = "Save";

// StatePanel
export const HOME_PANEL_ARIA = "How sites open right now";
export const HOME_PANEL_PICK_OTHER = "Choose another exit";
export const HOME_PANEL_DIRECT = "Open sites as usual";

// ThisComputerCard
export const HOME_COMPUTER_TITLE = "This computer";
export const HOME_COMPUTER_ALLOW = "Allow other members to open sites through this computer";
