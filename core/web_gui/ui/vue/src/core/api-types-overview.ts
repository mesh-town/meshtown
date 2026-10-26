/* Types of `GET /api/overview`, `GET /api/nets` and the leave reply, plus the per-network slice the page store keeps. */

import type { MembersReply } from "../members/members-types.ts";
import type { ConfigDocument, ConfigMeta } from "./api-types-config.ts";
import type { NodeState, Notice } from "./api-types-state.ts";

/** One network as listed by `nets`; field order follows the JSON. */
export interface NetBrief {
  id: string; // slot id: `main` or six characters of `[a-z2-7]`
  label: string | null;
  id_hex: string | null; // null — the slot holds no network key yet
  status: string;
  exit_enabled: boolean;
  proxy: boolean;
  route_state: string;
  exits: string[];
  joining: boolean;
  members_admitting: number;
  me_excluded: boolean;
  me_pending: boolean;
  peers_online: number;
  peers_total: number;
  me_admin?: boolean;
  can_invite?: boolean;
  admission_refused: boolean; // the server says the members turned this computer down (no delay: the page waits)
}

export interface SlotEntry {
  slot: string;
  state: NodeState | null;
  members: MembersReply | null;
}

export interface Overview {
  api: number;
  nets: NetBrief[];
  slots: SlotEntry[];
  restart_slots: string[];
  notices?: Notice[];
}

export type LeaveReply = MembersReply & { left: boolean; announced: boolean; nets: NetBrief[] };

/** Everything the page knows about one slot. */
export interface SlotSlice {
  id: string;
  brief: NetBrief;
  state: NodeState | null;
  members: MembersReply | null;
  config: ConfigDocument | null;
  configRev: number | null;
  configMeta: ConfigMeta | null;
  bootId: string | null;
}
