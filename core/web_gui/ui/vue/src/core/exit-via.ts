/* Exit choice across networks: the "Exit via …" buttons of "Members", "Site checks" and "Home".
 * The browser proxy follows one network (`brief.proxy`); an exit picked in another network moves it there. */

import type { SaveDeps } from "./save-config.ts";
import type { Store } from "./store.ts";
import { refetchConfig, saveConfigFields, saveSlotFields, fieldErrorText } from "./save-config.ts";
import type { ConfigChanges } from "./save-config.ts";
import { isMulti, visibleSlots } from "./slots.ts";
import { addRememberedFor, chosenExitIds } from "../common/exits.ts";
import { exitValue } from "../home/exit-list.ts";
import { api as coreApi } from "./api.ts";
import { CORE_SAVED } from "./core-texts.ts";

// `deps` — as for `saveConfigFields` plus `remember(exits)` (by default the page's localStorage).
export interface ExitSaveStatus {
  text: string;
  cls: string;
}

export type ExitDeps = SaveDeps & { remember?: (exits: string[]) => void };

const briefOf = (store: Store, slot: string) => store.nets.find((n) => n.id === slot);

// The shown slot the browser proxy goes through; `null` when it points to no shown network.
export function proxySlotOf(store: Store): string | null {
  const ids = visibleSlots(store.nets, store.slots);
  return ids.find((id) => briefOf(store, id)?.proxy === true) ?? null;
}

// The exit the proxy uses now. One network: the page config (it is the freshest), else the summary;
// several: the proxy slot's summary.
function currentExit(store: Store): { slot: string; id: string } | null {
  const slot = isMulti(store) ? proxySlotOf(store) : store.lead;
  if (slot === null) return null;
  const id = (isMulti(store) ? undefined : chosenExitIds(store)[0]) ?? briefOf(store, slot)?.exits?.[0];
  return id === undefined ? null : { slot, id };
}

export const exitChoiceValue = (store: Store): string => {
  const cur = currentExit(store);
  return cur === null ? "" : exitValue(cur.slot, cur.id, isMulti(store));
};

// Remembers the exit in play so that the row stays in the list after "Directly".
export function rememberCurrentExit(store: Store, remember: (slot: string, ids: string[]) => void): void {
  const cur = currentExit(store);
  if (cur !== null) remember(cur.slot, [cur.id]);
}

const statusOf = (ok: boolean, error: Parameters<typeof fieldErrorText>[0] | null, changes: ConfigChanges): ExitSaveStatus =>
  ok || error === null ? { text: CORE_SAVED, cls: "saved" } : { text: fieldErrorText(error, changes), cls: "field-error-text" };

async function putProxyNet(store: Store, slot: string, deps: ExitDeps): Promise<ExitSaveStatus> {
  const changes = { "proxy.net": slot };
  const res = await saveConfigFields(store, changes, deps);
  return statusOf(res.ok, res.ok ? null : res.error, changes);
}

// The slot already lists this exit but the proxy is elsewhere: the server sees no change in `route.exits`.
const needsProxyMove = (store: Store, slot: string, idFull: string): boolean => {
  const brief = briefOf(store, slot);
  return isMulti(store) && !!brief && !brief.proxy && brief.exits.length === 1 && brief.exits[0] === idFull;
};

export async function saveExitIn(store: Store, slot: string, idFull: string, deps: ExitDeps = {}): Promise<ExitSaveStatus> {
  const remember = deps.remember || ((ids: string[]) => addRememberedFor(slot, ids));
  const changes = { "route.exits": [idFull] };
  const move = needsProxyMove(store, slot, idFull);
  const hadMainExits = chosenExitIds(store).length > 0;
  const res = move ? await putProxyNet(store, slot, deps) : await writeExit(store, slot, changes, deps);
  if (res.cls !== "saved") return res;
  remember([idFull]);
  if (slot !== "main" || (move && hadMainExits)) await rereadMain(store, deps);
  return res;
}

// The node rewrites the primary config while it moves the proxy; its reply to the write does not carry the new revision.
async function rereadMain(store: Store, deps: ExitDeps): Promise<void> {
  try {
    await refetchConfig(store, { api: deps.api ?? coreApi });
  } catch (err) {
    /* the next save refetches on a stale revision anyway */
  }
}

async function writeExit(store: Store, slot: string, changes: ConfigChanges, deps: ExitDeps): Promise<ExitSaveStatus> {
  const res = slot === "main" ? await saveConfigFields(store, changes, deps) : await saveSlotFields(store, slot, changes, deps);
  return statusOf(res.ok, res.ok ? null : res.error, changes);
}

// "Directly": no exit; if the proxy sits in another network it comes back to the primary one in the same PUT.
export async function saveDirect(store: Store, deps: SaveDeps = {}): Promise<ExitSaveStatus> {
  const away = isMulti(store) && proxySlotOf(store) !== "main";
  const changes: ConfigChanges = away ? { "route.exits": [], "proxy.net": "main" } : { "route.exits": [] };
  const res = await saveConfigFields(store, changes, deps);
  return statusOf(res.ok, res.ok ? null : res.error, changes);
}

export const saveSingleExit = (store: Store, idFull: string, deps: ExitDeps = {}): Promise<ExitSaveStatus> =>
  saveExitIn(store, store.lead, idFull, deps);
