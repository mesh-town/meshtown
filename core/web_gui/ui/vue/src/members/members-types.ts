/* Types of `GET /api/members` and `POST /api/members/<action>` (the member-list API contract). */

export type MemberStatus = "member" | "pending" | "unlisted" | "excluded";

export type MemberAction = "exclude" | "unexclude" | "block" | "unblock" | "leave" | "admit";

// A join request that waits for an admin's confirmation.
export interface AdmissionInfo {
  label: string | null; // "for whom" label given with the invite
}

export interface ExcludedInfo {
  how: "removed" | "left";
  at_unix: number;
  /** The member whose record excluded the node (the node itself after a leave); `null` when the node does not say. */
  by: string | null;
  /** The note given with the removal: text, `null` when none was given, absent when the node does not say. */
  reason?: string | null;
}

export interface RefusalInfo {
  side: "me" | "them";
  reason: string;
  at_unix: number;
}

export interface MemberRow {
  id: string;
  id_full: string;
  name: string | null;
  self: boolean;
  online: boolean;
  status: MemberStatus;
  excluded: ExcludedInfo | null;
  blocked_by_me: boolean;
  denied_by_me: string | null;
  last_refusal: RefusalInfo | null;
  admission: AdmissionInfo | null;
  last_seen_sec: number | null; // seconds since the node was last online; null — never seen
  admin?: boolean;
}

export interface MemberCounts {
  members: number;
  excluded: number;
  unlisted: number;
  pending: number;
}

export interface MembersReply {
  me: string;
  me_excluded: boolean;
  me_pending: boolean; // this node waits for members to confirm it
  me_admission: AdmissionInfo | null;
  counts: MemberCounts;
  members: MemberRow[];
  changed?: boolean; // only in an action reply
  admins?: string[];
  me_admin?: boolean;
}

// An item of the `⋯` menu.
export interface MenuItem {
  key: "block" | "unblock" | "exclude" | "leave" | "rename" | "make-admin" | "make-member" | "give-up" | "delete-network";
  label: string;
  danger?: boolean;
  disabled?: boolean;
  title?: string; // why the item is disabled
}
