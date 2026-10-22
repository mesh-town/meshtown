/* Types of `GET /api/members` and `POST /api/members/<action>` (the member-list API contract). */

export type MemberStatus = "member" | "pending" | "unlisted" | "excluded";

export type MemberAction = "exclude" | "unexclude" | "block" | "unblock" | "leave" | "admit" | "unadmit";

// A pending removal vote on a row.
export interface VoteInfo {
  votes: number;
  need: number;
  proposed_by: string; // id_full of the proposer
  voters: string[];
  mine: boolean; // my node has voted
  reason: string | null; // why the proposer wants the removal
}

// A join request that has not reached the quorum yet (or a member held only by confirmations).
export interface AdmissionInfo {
  votes: number;
  need: number;
  voters: string[];
  mine: boolean; // my node has confirmed
  label: string | null; // "for whom" label given with the invite
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
}

export interface MemberCounts {
  members: number;
  voting: number;
  excluded: number;
  unlisted: number;
  pending: number;
}

export interface MembersReply {
  me: string;
  me_excluded: boolean;
  me_pending: boolean; // this node waits for members to confirm it
  me_admission: AdmissionInfo | null;
  need: number;
  counts: MemberCounts;
  members: MemberRow[];
  changed?: boolean; // only in an action reply
}

// An item of the `⋯` menu.
export interface MenuItem {
  key: "block" | "unblock" | "exclude" | "leave" | "rename" | "waiting" | "withdraw" | "unadmit";
  label: string;
  danger?: boolean;
  disabled?: boolean;
  title?: string; // why the item is disabled
}
