/* Sub-tabs of a network tab in "Networks": the type, their order, the address tail and the phone app's subset. */

import { NET_SUB_DEVICE, NET_SUB_MEMBERS, NET_SUB_SERVICES } from "../peers/peers-texts.ts";

export type NetSub = "members" | "services" | "device";

export const NET_SUBS: NetSub[] = ["members", "services", "device"];

const LABELS: Record<NetSub, string> = { members: NET_SUB_MEMBERS, services: NET_SUB_SERVICES, device: NET_SUB_DEVICE };

// Sub-tabs the phone app does not show.
const APP_HIDDEN: NetSub[] = ["services"];

export const netSubViews = (inApp: boolean): { id: NetSub; label: string }[] =>
  NET_SUBS.filter((id) => !(inApp && APP_HIDDEN.includes(id))).map((id) => ({ id, label: LABELS[id] }));

const SUB_TAIL = /^peers\/(?:main|[a-z2-7]{6})\/(members|services|device)$/;

// The sub-tab named by the address hash: `#peers/<tab>/<sub>`; the old `#ports` link means "services"; anything else — "members".
export function peersSub(hash: string | null | undefined): NetSub {
  const h = (hash || "").replace(/^#/, "");
  if (h === "ports") return "services";
  const m = SUB_TAIL.exec(h);
  return m ? (m[1] as NetSub) : "members";
}

// The sub-tab to show: one the phone app hides gives way to "members".
export const pickNetSub = (wanted: NetSub, inApp: boolean): NetSub => (inApp && APP_HIDDEN.includes(wanted) ? "members" : wanted);
