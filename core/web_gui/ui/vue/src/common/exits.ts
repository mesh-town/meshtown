import type { NodeState, ConfigDocument } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import { isSlotId } from "../core/slots.ts";

/* Chosen exits: the pure parts of the choice and the key
 * `meshtown.exitsRemembered` — the page's shared contract. */

export const REMEMBERED_KEY = "meshtown.exitsRemembered";

export const AUTO_PICKED_KEY = "meshtown.exitAutoPicked";

type ExitsStorage = Pick<Storage, "getItem" | "setItem">;

interface RememberedExit {
  id: string;
  slot: string;
}

function parseEntry(item: unknown): RememberedExit | null {
  if (typeof item === "string") return { id: item, slot: "main" };
  if (typeof item !== "object" || item === null) return null;
  const { id, slot } = item as { id?: unknown; slot?: unknown };
  return typeof id === "string" && typeof slot === "string" && isSlotId(slot) ? { id, slot } : null;
}

export function getRememberedEntries(storage: ExitsStorage = localStorage): RememberedExit[] {
  try {
    const raw = storage.getItem(REMEMBERED_KEY);
    const value: unknown = raw === null ? null : JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.map(parseEntry).filter((e): e is RememberedExit => e !== null);
  } catch (err) {
    return [];
  }
}

// Every entry is written as an object; a bare string in storage is a legacy `main` entry (read, never written).
export function setRememberedEntries(entries: RememberedExit[], storage: ExitsStorage = localStorage): void {
  try {
    const items = entries.map((e) => ({ id: e.id, slot: e.slot }));
    storage.setItem(REMEMBERED_KEY, JSON.stringify(items));
  } catch (err) {
    /* storage unavailable — the choice simply won't be remembered */
  }
}

export function getRememberedFor(slot: string, storage: ExitsStorage = localStorage): string[] {
  return getRememberedEntries(storage)
    .filter((e) => e.slot === slot)
    .map((e) => e.id);
}

// The remembered ids of a slot told by origin: `trusted` were written as objects by this page, `legacy` are bare
// strings (the `main` memory of older pages, which could hold an id that never was a member).
export function getRememberedSplit(slot: string, storage: ExitsStorage = localStorage): { trusted: string[]; legacy: string[] } {
  const all = getRememberedFor(slot, storage);
  const legacy = slot === "main" ? bareStrings(storage) : [];
  return { trusted: all.filter((id) => !legacy.includes(id)), legacy: all.filter((id) => legacy.includes(id)) };
}

function bareStrings(storage: ExitsStorage): string[] {
  try {
    const raw = storage.getItem(REMEMBERED_KEY);
    const value: unknown = raw === null ? null : JSON.parse(raw);
    return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
  } catch (err) {
    return [];
  }
}

// Only the entries of `slot` are replaced; other networks' entries stay.
export function setRememberedFor(slot: string, ids: string[], storage: ExitsStorage = localStorage): void {
  const others = getRememberedEntries(storage).filter((e) => e.slot !== slot);
  const own = ids.map((id) => ({ id, slot }));
  setRememberedEntries(slot === "main" ? [...own, ...others] : [...others, ...own], storage);
}

const REMEMBERED_MAX = 8;

// Appends the ids that the slot does not hold yet: the order of the existing entries stays, the oldest go over the limit.
export function addRememberedFor(slot: string, ids: string[], storage: ExitsStorage = localStorage): void {
  const have = getRememberedFor(slot, storage);
  const added = ids.filter((id, i) => !have.includes(id) && ids.indexOf(id) === i);
  setRememberedFor(slot, [...have, ...added].slice(-REMEMBERED_MAX), storage);
}

export function getRememberedExits(storage: ExitsStorage = localStorage): string[] {
  return getRememberedFor("main", storage);
}

export function setRememberedExits(exits: string[], storage: ExitsStorage = localStorage): void {
  setRememberedFor("main", exits, storage);
}

export function configExits(config: ConfigDocument | null | undefined): string[] {
  return (config && config.route && config.route.exits) || [];
}

// The page config is no older than the node's route snapshot — take the config; a snapshot older than the config —
// the snapshot (a tray edit or another tab).
export function pickChosenExits(state: NodeState | null, configList: string[], configRev: number | null): string[] {
  if (!state || !state.route) return configList;
  const route = state.route;
  const fromRoute = () => route.exits.map((e) => e.id_full);
  if (configRev === null) return fromRoute();
  return state.config_rev <= configRev ? configList : fromRoute();
}

export function chosenExitIds(store: Store): string[] {
  return pickChosenExits(store.state, configExits(store.config), store.configRev);
}

export function wasExitAutoPicked(storage: ExitsStorage | null): boolean {
  try {
    return storage!.getItem(AUTO_PICKED_KEY) !== null;
  } catch (err) {
    return true; // no storage — the pick could not be made once, so it is not made at all
  }
}

export function markExitAutoPicked(storage: ExitsStorage | null): void {
  try {
    storage!.setItem(AUTO_PICKED_KEY, "1");
  } catch (err) {
    /* storage unavailable */
  }
}
