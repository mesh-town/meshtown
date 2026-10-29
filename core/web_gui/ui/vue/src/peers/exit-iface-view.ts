/* The "Send exit traffic through" list and the state line under it. */

import type { ExitIfaceInfo } from "../core/api-types-exit.ts";
import type { NetEntry } from "../core/api-types-state.ts";
import { ifaceTitle } from "../lan/iface-title.ts";
import { EXIT_IFACE_ANY, EXIT_IFACE_NOT_FOUND_SUFFIX, exitIfaceDown, exitIfaceMissing, exitIfaceUnsupported } from "./exit-iface-texts.ts";

const isTunnel = (e: NetEntry): boolean => e.kind === "tunnel";

function labelOf(e: NetEntry, all: NetEntry[]): string {
  const title = ifaceTitle(e, false, all).title;
  return title ? `${title} (${e.name})` : e.name;
}

/** `[value, label]` pairs: "any network" first, then tunnels, then the other networks; a chosen name that is not in the list is kept, marked. */
export function ifaceOptions(networks: NetEntry[], current: string | null): [string, string][] {
  const sorted = [...networks.filter(isTunnel), ...networks.filter((e) => !isTunnel(e))];
  const list: [string, string][] = sorted.map((e) => [e.name, labelOf(e, networks)]);
  const missing = current && !networks.some((e) => e.name === current);
  return [["", EXIT_IFACE_ANY], ...list, ...(missing ? [[current, current + EXIT_IFACE_NOT_FOUND_SUFFIX] as [string, string]] : [])];
}

const STATE_TEXT: Record<string, (name: string) => string> = {
  missing: exitIfaceMissing,
  down: exitIfaceDown,
  unsupported: exitIfaceUnsupported,
};

/** A warning for an interface that cannot carry the traffic; `null` — nothing to say. */
export function ifaceStateText(info: ExitIfaceInfo | null | undefined): string | null {
  return info ? (STATE_TEXT[info.state]?.(info.name) ?? null) : null;
}
