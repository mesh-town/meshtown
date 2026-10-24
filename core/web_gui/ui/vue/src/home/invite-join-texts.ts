/* Texts of the one "Join with an invite" form: the title, the note under the field and the limit lines. */

import { MAX_NETS } from "./add-net-texts.ts";

export const INVITE_FORM_TITLE = "Join with an invite";

/** The placeholder for a network whose name is unknown; it is not put in quotes. */
export const NOTE_THIS_NET = "this network";

const named = (net: string): string => (net === NOTE_THIS_NET ? net : `"${net}"`);

export const NOTE_HERE = (net: string): string => `You're already in ${named(net)}.`;
export const NOTE_REMOVED = (net: string): string => `This invite brings this computer back to ${named(net)}.`;
export const NOTE_EXCLUDED_MANAGED = (net: string): string =>
  `This invite brings this computer back to ${named(net)} if an admin of ${named(net)} made it.`;
export const NOTE_BANNED_PEER = (net: string): string =>
  `${named(net)} won't take this computer back — its members will refuse this invite. It won't be used up.`;
export const NOTE_NEW_AT_LIMIT = `This is a new network, and Mesh Town is already in ${MAX_NETS}. Leave a network first.`;

export const ADD_NET_LIMIT_INVITE =
  `Mesh Town is in ${MAX_NETS} networks, the most it can be in. To join a new one, leave a network first. You can still rejoin one of these.`;

export const REJOIN_TITLE = (net: string): string => `Join ${named(net)} again`;
export const REJOIN_OTHER = "Join another network";
