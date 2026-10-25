/* Texts of the network kind choice on the create forms and of the line that describes a pasted invite. */

import { forDevice } from "../common/device.ts";

export const KIND_TITLE = "Who can invite and remove members?";
export const KIND_PEER_TITLE = "Peer network";
export const KIND_PEER_TEXT = "Any member can invite people. Removing someone takes two members' votes.";
export const KIND_MANAGED_TITLE = "Managed network";
export const KIND_MANAGED_TEXT = "Only admins invite and remove members. You become the first admin.";
export const KIND_FINAL_HINT = "You can't change this after the network is created.";

/** The warning under a chosen Managed card; `name` — what is typed in the name field (blank — not named yet). */
export function managedWarning(name: string): string {
  const net = name.trim() ? `"${name.trim()}"` : "this network";
  return forDevice(`If this computer's Mesh Town data is lost and nobody else is an admin, ${net} can't be managed any more. Make a second admin soon.`);
}

const named = (label: string | null): string => (label ? `Network "${label}"` : "Network");

export const previewManaged = (label: string | null): string => `${named(label)} · Managed: admins decide who joins`;
export const previewPeer = (label: string | null): string => `${named(label)} · Peer: every member can invite`;
