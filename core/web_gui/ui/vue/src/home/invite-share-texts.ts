/* Texts of the "share the invite" window that opens after "Create invite". */

import { HOME_JOIN_FIELD_LABEL } from "./home-card-texts.ts";
import { HOME_JOIN_SCAN } from "./invite-qr-texts.ts";

export const inviteShareTitle = (name: string): string => (name.trim() ? "Invite for " + name.trim() : "Invite ready");
export const INVITE_SHARE_LEAD = "Tell the new member to do this:";
export const INVITE_SHARE_INSTALL = "Install Mesh Town";
export const INVITE_SHARE_OPEN = "Open it and choose to join a network.";
export const INVITE_SHARE_PASTE = "Paste the invite into “" + HOME_JOIN_FIELD_LABEL + "”.";
export const INVITE_SHARE_SCAN = "On Android tap “" + HOME_JOIN_SCAN + "” and point the camera here.";
export const INVITE_SHARE_NOTE = "Works once, for 24 hours. Send it only to this person.";
export const INVITE_SHARE_GROUP = "How to send the invite";
export const INVITE_SHARE_AS_TEXT = "Send as text";
export const INVITE_SHARE_AS_QR = "Send as QR code";
export const INVITE_SHARE_COPY = "Copy";
export const INVITE_SHARE_COPIED = "Copied";
export const INVITE_SHARE_MANUAL = "Select and copy it by hand";
export const INVITE_SHARE_CLOSE = "Done";
