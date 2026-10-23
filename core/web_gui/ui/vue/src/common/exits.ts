import type { NodeState, ConfigDocument } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import { isSlotId } from "../core/slots.ts";

/* Chosen exits: a copy of the pure parts of `14-home-exit-choice.js` and of the key
 * `meshtown.exitsRemembered` — the page's shared contract (formerly shared with the classic page). */

export const REMEMBERED_KEY = "meshtown.exitsRemembered";

export const AUTO_PICKED_KEY = "meshtown.exitAutoPicked";

type ExitsStorage = Pick<Storage, "getItem" | "setItem">;

export interface RememberedExit {
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

// `main` entries are written as plain strings, so a single network stores exactly what older pages stored.
export function setRememberedEntries(entries: RememberedExit[], storage: ExitsStorage = localStorage): void {
  try {
    const items = entries.map((e) => (e.slot === "main" ? e.id : { id: e.id, slot: e.slot }));
    storage.setItem(REMEMBERED_KEY, JSON.stringify(items));
  } catch (err) {
    /* storage unavailable — the choice simply won't be remembered */
  }
}

export function getRememberedExits(storage: ExitsStorage = localStorage): string[] {
  return getRememberedEntries(storage)
    .filter((e) => e.slot === "main")
    .map((e) => e.id);
}

export function setRememberedExits(exits: string[], storage: ExitsStorage = localStorage): void {
  const others = getRememberedEntries(storage).filter((e) => e.slot !== "main");
  setRememberedEntries([...exits.map((id) => ({ id, slot: "main" })), ...others], storage);
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
