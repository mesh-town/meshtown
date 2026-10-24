/* Home strings that appear with two or more networks. */

export const NETS_CARD_ARIA = "Your networks";
export const NETS_OPEN_MEMBERS = "Open Members";
export const NETS_NO_CONNECTION = "no connection";
export const NETS_JUST_YOU = "just you";
export const NETS_EXIT_ALL = "(one exit for all your networks)";
export const NETS_EXIT_NONE_IN = "No member of this network offers an exit yet.";
export const NETS_EXIT_NO_NET = "Sites won't open: you're no longer in the network of the chosen exit. Pick an exit or \"Directly\".";
export const NETS_EXIT_NOT_OFFERED = (name: string): string => `${name} no longer offers an exit, so sites won't open. Pick another exit or "Directly".`;
export const NETS_DIRECT = "Sites open directly";

export const netsOnlineText = (online: number, total: number): string => `${online} of ${total} online`;
export const netsAdmittingTail = (n: number): string => ` · ${n} ${n === 1 ? "request" : "requests"} to confirm`;
export const netsChipText = (n: number, net: string): string => `${n} ${n === 1 ? "request" : "requests"} to confirm in ${net}`;
export const netsChipShort = (n: number, net: string): string => `${n} to confirm · ${net}`;

export const netsViaText = (peer: string, net: string): string => `Sites open via ${peer} (${net})`;
export const netsExitRemovedText = (peer: string, net: string): string => `${peer} is no longer in "${net}" — sites won’t open through it.`;
export const netsNoLinkText = (peer: string, net: string): string => `No connection to "${net}" — sites won’t open through ${peer}.`;
export const netsExitOfflineText = (peer: string, net: string): string => `${peer} (${net}) is offline — sites won't open until it's back.`;
