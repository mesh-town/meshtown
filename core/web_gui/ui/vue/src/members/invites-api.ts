/* Client of `GET network/invites` and `POST network/invites/label`: the open invites of a managed network. */

import type { ApiFn } from "../core/api.ts";
import { slotPath } from "../core/slots.ts";
import { errReason } from "../common/words.ts";

export interface OpenInvite {
  hash: string;
  for_whom: string | null;
  made_unix: number;
  expires_unix: number;
  issuer: string | null;
  issuer_name: string | null;
  mine: boolean;
}

type Raw = Record<string, unknown>;

const isObj = (v: unknown): v is Raw => typeof v === "object" && v !== null && !Array.isArray(v);
const num = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) ? v : 0);
const text = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v : null);

function parseInvite(v: unknown): OpenInvite | null {
  if (!isObj(v) || typeof v.hash !== "string") return null;
  return {
    hash: v.hash,
    for_whom: text(v.for_whom),
    made_unix: num(v.made_unix),
    expires_unix: num(v.expires_unix),
    issuer: text(v.issuer),
    issuer_name: text(v.issuer_name),
    mine: v.mine === true,
  };
}

// Tolerant to extra fields; not a list of invites — null.
export function parseInvites(raw: unknown): OpenInvite[] | null {
  if (!isObj(raw) || !Array.isArray(raw.invites)) return null;
  return raw.invites.map(parseInvite).filter((i): i is OpenInvite => i !== null);
}

export async function getInvites(api: ApiFn, slot: string): Promise<OpenInvite[] | null> {
  return parseInvites(await api("GET", slotPath(slot, "network/invites")));
}

// An empty text clears the label; the reply is the list after the change.
export async function relabelInvite(api: ApiFn, slot: string, hash: string, label: string): Promise<OpenInvite[] | null> {
  return parseInvites(await api("POST", slotPath(slot, "network/invites/label"), { hash, for: label }));
}

const SENTENCES: Record<string, string> = {
  not_issuer: "Only the admin who made this invite can rename it.",
  not_admin: "Only admins can do this.",
  not_found: "This invite has been used or has expired.",
  invalid: "That label is not valid.",
  busy: "The network is starting, try again in a moment.",
};

// A human sentence for a failed call: the known code first, then the node's message.
export function inviteErrorText(err: unknown): string {
  const code = isObj(err) && typeof err.code === "string" ? err.code : "";
  const reason = errReason(err);
  return SENTENCES[code] || (reason ? "Couldn't update the invites: " + reason : "Couldn't update the invites — try again");
}
