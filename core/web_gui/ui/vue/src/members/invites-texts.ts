/* Texts of the "Pending invites" section. */

export const INVITES_TITLE = "Pending invites";
export const INVITES_EMPTY = "No open invites";
export const INVITES_NO_NAME = "no name";
export const INVITES_SAVE = "Save";
export const labelAria = "Who this invite is for";

const issueTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export const issuedAt = (unix: number): string => "issued " + issueTime.format(unix * 1000);
export const invitedBy = (mine: boolean, name: string | null): string => (mine ? "by you" : name ? "by " + name : "by another admin");
