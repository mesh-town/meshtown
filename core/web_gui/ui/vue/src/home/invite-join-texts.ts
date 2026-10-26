/* Texts of the one "Join with an invite" form: the title, the note under the field and the limit lines. */

import { forDevice } from "../common/device.ts";
import { MAX_NETS } from "./add-net-texts.ts";

export const INVITE_FORM_TITLE = "Join with an invite";

/** The placeholder for a network whose name is unknown; it is not put in quotes. */
export const NOTE_THIS_NET = "this network";

const named = (net: string): string => (net === NOTE_THIS_NET ? net : `"${net}"`);

export const NOTE_HERE = (net: string): string => `You're already in ${named(net)}.`;
export const NOTE_REMOVED = (net: string): string => `This invite brings ${forDevice("this computer")} back to ${named(net)}.`;
export const NOTE_EXCLUDED_MANAGED = (net: string): string =>
  `This invite brings ${forDevice("this computer")} back to ${named(net)} if an admin of ${named(net)} made it.`;
export const NOTE_NEW_AT_LIMIT = `This is a new network, and Mesh Town is already in ${MAX_NETS}, the most it can be in. Leave a network first.`;

/** The short error under the field of "Add" (the line above the field already says the rest). */
export const ADD_NET_LIMIT_HERE = "This is a new network, so it can't be added until you leave one.";

/** The bold lead of a refused join, before the core's own text; `netQuoted` is `quotedNet(label)` (with its leading space) or "". */
export const REFUSED_LEAD = (netQuoted: string): string => `Couldn't join${netQuoted}.`;

export const ADD_NET_LIMIT_INVITE =
  `Mesh Town is in ${MAX_NETS} networks, the most it can be in. To join a new one, leave a network first. You can still rejoin one of these.`;

export const REJOIN_TITLE = (net: string): string => `Join ${named(net)} again`;
export const REJOIN_OTHER = "Join another network";
