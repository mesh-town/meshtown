import type { NodeState, ConfigDocument } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";

/* Chosen exits: a copy of the pure parts of `14-home-exit-choice.js` and of the key
 * `meshtown.exitsRemembered` — the page's shared contract (formerly shared with the classic page). */

export const REMEMBERED_KEY = "meshtown.exitsRemembered";

export const AUTO_PICKED_KEY = "meshtown.exitAutoPicked";

type ExitsStorage = Pick<Storage, "getItem" | "setItem">;

export function getRememberedExits(storage: ExitsStorage = localStorage): string[] {
  try {
    const raw = storage.getItem(REMEMBERED_KEY);
    const value: unknown = raw === null ? null : JSON.parse(raw);
    return Array.isArray(value) ? value : [];
  } catch (err) {
    return [];
  }
}

export function setRememberedExits(exits: string[], storage: ExitsStorage = localStorage): void {
  try {
    storage.setItem(REMEMBERED_KEY, JSON.stringify(exits));
  } catch (err) {
    /* storage unavailable — the choice simply won't be remembered */
  }
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
