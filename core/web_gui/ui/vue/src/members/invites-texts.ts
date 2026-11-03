/* Texts of the "Pending invites" section. */

export const INVITES_TITLE = "Pending invites";
export const INVITES_EMPTY = "No open invites";
export const INVITES_NO_NAME = "no name";
export const INVITES_SAVE = "Save";
export const labelAria = "Who this invite is for";

export const issuedAgo = (ago: string): string => "issued " + ago;
export const invitedBy = (mine: boolean, name: string | null): string => (mine ? "by you" : name ? "by " + name : "by another admin");
