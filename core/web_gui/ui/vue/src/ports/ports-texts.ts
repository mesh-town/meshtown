// Texts of the "Ports" section: publications, network ports, form captions and errors.

export const PORTS_ADD_ERROR_RANGE = "A port is a number from 1 to 65535.";
export const PORTS_ADD_ERROR_NO_NODES = "Select at least one member.";
export const PORTS_EMPTY_OWN = "— nothing published";
export const PORTS_EMPTY_NET = "— no member has shared ports with you yet";

export const PORTS_STATUS_OK = "responding";
export const PORTS_STATUS_REFUSED = "not responding";
export const PORTS_STATUS_UNKNOWN = "not checked yet";

export const PORTS_PROTO_LABEL = "protocol";
export const PORTS_PROTO_TCP = "TCP";
export const PORTS_PROTO_UDP = "UDP";
export const PORTS_STATUS_UDP = "not checked for UDP";

const portPhrase = (port: number, proto: "tcp" | "udp"): string => (proto === "udp" ? "UDP port " : "Port ") + port;

export function portsAlreadyPublishedText(port: number, proto: "tcp" | "udp" = "tcp"): string {
  return portPhrase(port, proto) + " is already published.";
}

export function portsNotPublishedText(port: number, proto: "tcp" | "udp" = "tcp"): string {
  return portPhrase(port, proto) + " is not published.";
}

export const PORTS_LAST_ERROR_TEXT = {
  unsupported: "The member runs an old version: UDP is not supported",
  offline: "The member is offline",
  not_opened: "The member has not shared this port with you",
  no_service: "Nothing is listening on the member's port",
  timed_out: "The member did not answer in time",
  failed: "Couldn't reach the member's port",
} as const;

export const PORTS_AUDIENCE_ALL = "all members";
export const PORTS_AUDIENCE_NOBODY = "nobody";

export function portsBindFailedText(local: number): string {
  return "port " + local + " is busy — choose another number";
}

export const PORTS_OFFLINE = "offline";
export const PORTS_ONLINE = "online";

export function portsOpenErrorText(error: string): string {
  return "Couldn't open: " + error;
}

export function portsNodeLabelText(node: string): string {
  return "member " + node;
}

export const PORTS_NO_STORE = "PortsSection: store not provided (STORE_KEY)";
export const PORTS_MINE_TITLE = "My published ports";
export const PORTS_NET_TITLE = "Ports shared with you";
export const PORTS_COL_PORT = "Port";
export const PORTS_COL_NAME = "Name";
export const PORTS_COL_BROWSER = "In the browser";
export const PORTS_COL_PROGRAMS = "In other programs";
export const PORTS_COL_AUDIENCE = "Shared with";
export const PORTS_COL_STATUS = "Status";
export const PORTS_COL_PEER = "Member";
export const PORTS_MINE_HINT = "Publish the port of a program that runs on this computer (address 127.0.0.1). All network members, or only the members you select, will see it and can open it — it isn't shown to anyone else and won't open for them. You can change the name and \"Shared with\" with the \"Edit\" button.";
export const PORTS_NET_HINTS = [
  "Click an address to open it in this browser. It works in a browser set up to use Mesh Town (see the Home tab). Type addresses with http:// at the start, or the browser may search the web instead.",
  "Mesh Town can't explain errors on https:// addresses: if one doesn't open, the browser shows its own error. To see why, open the same address with http:// at the start and :443 at the end — for example, http://home-openwrt.meshtown:443.",
  "For other programs — for example, an SSH or file-sharing client — click “Open on this computer”: the port becomes available at 127.0.0.1, by default with the member's port plus 2000 (80 → 2080).",
];
export const PORTS_SAME_NAME_HINT = "Some members have the same name, so their addresses end with the start of the member's id.";
export const PORTS_SAME_NAME_TITLE = "Another member has the same name, so this address ends with the start of this member's id.";
export const PORTS_NOT_FOR_BROWSER = "not for the browser";
export const PORTS_NOT_FOR_BROWSER_TITLE = "Browsers don't open this port. Use “Open on this computer” and a program for it.";
export const PORTS_COPY = "Copy";
export const PORTS_COPIED = "Copied";

export function portsCopyAddressLabel(address: string): string {
  return "Copy address " + address;
}

export const PORTS_AUDIENCE_LABEL = "Shared with";
export const PORTS_AUDIENCE_PICK_ALL = "All members";
export const PORTS_AUDIENCE_PICK_NODES = "Selected members";
export const PORTS_NO_PEERS = "There are no other members in the network yet";

export const PORTS_CHANGE_NUMBER = "Change number";
export const PORTS_LOCAL_PORT_LABEL = "port number on this computer";

export function portsLocalAddress(local: number): string {
  return "127.0.0.1:" + local;
}

export const PORTS_LOCAL_PORT_TITLE = "port number on this computer";
export const PORTS_OPEN = "Open on this computer";
export const PORTS_OPEN_TITLE = "Make this port available at 127.0.0.1 for programs other than the browser";
export const PORTS_CLOSE = "Close";
export const PORTS_OPENED = "open";
export const PORTS_EDIT = "Edit";
export const PORTS_UNPUBLISH = "Unpublish";

export function portsEditTitleText(port: number, proto: "tcp" | "udp" = "tcp"): string {
  return proto === "udp" ? "Edit UDP port " + port : "Edit port " + port;
}

export const PORTS_NAME_PLACEHOLDER = "name (optional)";
export const PORTS_SAVE = "Save";
export const PORTS_CANCEL = "Cancel";
export const PORTS_PORT_PLACEHOLDER = "port";
export const PORTS_PUBLISH = "Publish";

export const PORTS_NET_ALL = "all";
export const PORTS_NET_SEPARATOR = " · ";

export function portsLocalUsedText(local: number, by: string): string {
  return local + " is already used by " + by;
}

export function portsNetPeerText(peer: string, net: string): string {
  return peer + " (" + net + ")";
}

export function portsNetGroupText(net: string): string {
  return net + ":";
}
