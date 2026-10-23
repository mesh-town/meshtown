// Texts of the "Ports" section: publications, network ports, form captions and errors.

export const PORTS_ADD_ERROR_RANGE = "A port is a number from 1 to 65535.";
export const PORTS_ADD_ERROR_NO_NODES = "Select at least one member.";
export const PORTS_EMPTY_OWN = "— nothing published";
export const PORTS_EMPTY_NET = "— other members haven't published anything yet";

export const PORTS_STATUS_OK = "responding";
export const PORTS_STATUS_REFUSED = "not responding";
export const PORTS_STATUS_UNKNOWN = "not checked yet";

export function portsAlreadyPublishedText(port: number): string {
  return "Port " + port + " is already published.";
}

export function portsNotPublishedText(port: number): string {
  return "Port " + port + " is not published.";
}

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
export const PORTS_NET_TITLE = "Ports shared by members";
export const PORTS_COL_PORT = "Port";
export const PORTS_COL_NAME = "Name";
export const PORTS_COL_AUDIENCE = "Shared with";
export const PORTS_COL_STATUS = "Status";
export const PORTS_COL_PEER = "Member";
export const PORTS_COL_ONLINE = "Online";
export const PORTS_MINE_HINT = "Publish the port of a program that runs on this computer (address 127.0.0.1). All network members, or only the members you select, will see it and can open it — it isn't shown to anyone else and won't open for them. You can change the name and \"Shared with\" with the \"Edit\" button.";
export const PORTS_NET_HINT = "Click \"Open\" to make a member's port available on this computer at 127.0.0.1 with the number you choose. By default, it's the member's port plus 2000 (80 → 2080).";

export const PORTS_AUDIENCE_LABEL = "Shared with";
export const PORTS_AUDIENCE_PICK_ALL = "All members";
export const PORTS_AUDIENCE_PICK_NODES = "Selected members";
export const PORTS_NO_PEERS = "There are no other members in the network yet";

export const PORTS_LOCAL_PORT_TITLE = "port number on this computer";
export const PORTS_OPEN = "Open";
export const PORTS_CLOSE = "Close";
export const PORTS_OPENED = "open";
export const PORTS_EDIT = "Edit";
export const PORTS_UNPUBLISH = "Unpublish";

export function portsEditTitleText(port: number): string {
  return "Edit port " + port;
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
