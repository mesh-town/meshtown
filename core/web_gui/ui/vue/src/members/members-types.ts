/* Types of `GET /api/members` and `POST /api/members/<action>` (the member-list API contract). */

export type MemberStatus = "member" | "pending" | "unlisted" | "excluded" | "forgotten";

export type MemberAction = "exclude" | "unexclude" | "block" | "unblock" | "leave" | "admit" | "unadmit" | "forget" | "unforget";

// A pending ban vote on a row.
export interface VoteInfo {
  votes: number;
  need: number;
  proposed_by: string; // id_full of the proposer
  voters: string[];
  mine: boolean; // my node has voted
  reason: string | null; // why the proposer wants the ban
}

// A join request that has not reached the quorum yet (or a member held only by confirmations).
export interface AdmissionInfo {
  votes: number;
  need: number;
  voters: string[];
  mine: boolean; // my node has confirmed
  label: string | null; // "for whom" label given with the invite
}

// A removal circle on a row: members who asked to drop an offline node from the list (short ids).
export interface ForgetInfo {
  votes: number;
  need: number;
  proposed_by: string; // short id of the proposer
  voters: string[]; // short ids
  mine: boolean; // my node has voted
  at_unix: number | null; // set once the circle is complete
}

export interface ExcludedInfo {
  how: "votes" | "left";
  at_unix: number;
  voters: string[];
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
  vote: VoteInfo | null;
  excluded: ExcludedInfo | null;
  blocked_by_me: boolean;
  denied_by_me: string | null;
  last_refusal: RefusalInfo | null;
  admission: AdmissionInfo | null;
  forget: ForgetInfo | null;
  last_seen_sec: number | null; // seconds since the node was last online; null — never seen
}

export interface MemberCounts {
  members: number;
  voting: number;
  excluded: number;
  unlisted: number;
  pending: number;
  forgotten: number;
}

export interface MembersReply {
  me: string;
  me_excluded: boolean;
  me_removed: boolean; // this node was removed from the member list
  me_pending: boolean; // this node waits for members to confirm it
  me_admission: AdmissionInfo | null;
  need: number;
  counts: MemberCounts;
  members: MemberRow[];
  changed?: boolean; // only in an action reply
}

// An item of the `⋯` menu.
export interface MenuItem {
  key: "block" | "unblock" | "exclude" | "leave" | "rename" | "waiting" | "withdraw" | "unadmit" | "forget" | "unforget";
  label: string;
  danger?: boolean;
  disabled?: boolean;
  title?: string; // why the item is disabled
}
