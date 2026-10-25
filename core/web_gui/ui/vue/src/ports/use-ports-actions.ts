/* Actions of the "Ports" section: publish, unpublish, "Open" and "Close". The list is edited by the pure
 * functions of `ports-view.ts`, saved by `saveConfigFields`; the result stays with the form until the next edit. */

import { ref } from "vue";
import type { Ref } from "vue";
import type { Store } from "../core/store.ts";
import { notifyChanged } from "../core/bridge.ts";
import { eachSlot, fieldErrorText, saveConfigFields, saveSlotFields, slotFailureText, targetSlots } from "../core/save-config.ts";
import { isMulti, netConfig } from "../core/slots.ts";
import type { SaveDeps } from "../core/save-config.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { CORE_SAVED } from "../core/core-texts.ts";
import type { NetAudiences, PortsConfigKey, PortsConfigLists } from "./ports-view.ts";
import {
  ADD_ERROR_NO_NODES, configPorts, netRowKey, openedAfterClose, openedAfterOpen, publishedAfterAdd, publishedAfterEdit, publishedAfterRemove, takenLocals,
} from "./ports-view.ts";
import type { PublishedPortConfig } from "../core/api-types.ts";

// The node snapshot is rebuilt once a second: the second poll after saving catches the fresh one.
const SNAPSHOT_SETTLE_MS = 1200;

/** `deps` — as in `saveConfigFields`, plus `setTimer` to be replaced in tests. */
type PortsDeps = SaveDeps & { setTimer?: (fn: () => void, ms: number) => unknown };

/** One network's choice ("all" / "all" or ids) or, with several networks, the choice per network (`null` — not published there). */
export type PortAudience = "all" | string[] | NetAudiences;

const isNetAudiences = (a: PortAudience): a is NetAudiences => a !== "all" && !Array.isArray(a);
type SlotList = { slot: string; list: PublishedPortConfig[] };

interface PortsActions {
  saveStatus: Ref<ExitSaveStatus>;
  addError: Ref<string>;
  editError: Ref<string>;
  saving: Ref<boolean>;
  /** Refusals of "Open" and "Close" by the row key (`netRowKey`). */
  openErrors: Ref<Record<string, string>>;
  publish: (portValue: unknown, nameValue: unknown, audience?: PortAudience) => Promise<boolean>;
  edit: (port: number, nameValue: unknown, audience: PortAudience) => Promise<boolean>;
  unpublish: (port: number) => Promise<boolean>;
  openNetPort: (slot: string, node: string, port: number, local?: unknown) => Promise<boolean>;
  closeNetPort: (slot: string, node: string, port: number) => Promise<boolean>;
}

function defaultBusTarget(): EventTarget | undefined {
  return typeof document === "undefined" ? undefined : document;
}

// `deps` — as in `saveConfigFields` (`{ api, busTarget }`); in tests a fake `api`.
export function usePortsActions(store: Store, deps?: PortsDeps): PortsActions {
  const saveStatus = ref<ExitSaveStatus>({ text: "", cls: "" });
  const addError = ref<string>("");
  const editError = ref<string>("");
  // A save is in progress: a second action does not start meanwhile (the buttons are `disabled`), otherwise the list
  // would be built from a stale store and overwrite the previous edit.
  const saving = ref<boolean>(false);
  const openErrors = ref<Record<string, string>>({});

  // The list from the store is a reactive proxy; `saveConfigFields` unwraps the proxy itself (`deepRaw`),
  // so a plain array goes into the request.
  function current<K extends PortsConfigKey>(key: K): PortsConfigLists[K] {
    return configPorts(store.config, key);
  }

  function notifyState(): void {
    const target = (deps && deps.busTarget) || defaultBusTarget();
    if (target) notifyChanged(target, "state");
  }

  // Poll at once and once more when the node snapshot has had time to update.
  function refreshState(): void {
    const setTimer = (deps && deps.setTimer) || ((fn: () => void, ms: number) => setTimeout(fn, ms));
    notifyState();
    setTimer(notifyState, SNAPSHOT_SETTLE_MS);
  }

  // A refusal goes to the shared status, or to `refusal` when the caller shows it elsewhere.
  async function doSave(key: PortsConfigKey, list: PortsConfigLists[PortsConfigKey], refusal?: Ref<string>): Promise<boolean> {
    const changes = { ["ports." + key]: list };
    const res = await saveConfigFields(store, changes, deps);
    if (!res.ok && refusal) refusal.value = fieldErrorText(res.error, changes);
    else saveStatus.value = res.ok
      ? { text: CORE_SAVED, cls: "saved" }
      : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
    // The rows of "My published ports" and "Network ports" come from the state: we ask the bridge to poll at once and once more a bit later.
    if (res.ok) refreshState();
    return res.ok;
  }

  // Busy — false without a request; the flag is also cleared on an exception.
  async function save(key: PortsConfigKey, list: PortsConfigLists[PortsConfigKey], refusal?: Ref<string>): Promise<boolean> {
    if (saving.value) return false;
    saving.value = true;
    try {
      return await doSave(key, list, refusal);
    } finally {
      saving.value = false;
    }
  }

  // Success — true (the form is cleared); a validation error — the text in `addError`, no request.
  async function publish(portValue: unknown, nameValue: unknown, audience: PortAudience = "all"): Promise<boolean> {
    if (saving.value) return false;
    const port = Number(portValue);
    const name = String(nameValue ?? "").trim();
    if (isNetAudiences(audience)) return publishNets(port, name, audience);
    const res = publishedAfterAdd(current("published"), port, name, audience);
    addError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list);
  }

  // Editing the name and "shared with": a validation error — the text in `editError`, no request.
  async function edit(port: number, nameValue: unknown, audience: PortAudience): Promise<boolean> {
    if (saving.value) return false;
    const name = String(nameValue ?? "").trim();
    if (isNetAudiences(audience)) return editNets(port, name, audience);
    const res = publishedAfterEdit(current("published"), port, name, audience);
    editError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list, editError);
  }

  function unpublish(port: number): Promise<boolean> {
    if (isMulti(store)) return writeNets(targetSlots(store).map((slot) => ({ slot, list: publishedAfterRemove(listOf(slot), port) })));
    return save("published", publishedAfterRemove(current("published"), port));
  }

  // ---- Several networks: each network gets its own list, only the changed ones are written ----

  const listOf = (slot: string): PublishedPortConfig[] => configPorts(netConfig(store, slot), "published");

  // One network's new list for `audience`: null — not published there.
  function listFor(slot: string, port: number, name: string, audience: "all" | string[] | null | undefined, adding: boolean) {
    const list = listOf(slot);
    const has = list.some((p) => p.port === port);
    if (audience == null) return { list: has ? publishedAfterRemove(list, port) : list };
    return has && !adding ? publishedAfterEdit(list, port, name, audience) : publishedAfterAdd(list, port, name, audience);
  }

  function planNets(port: number, name: string, audiences: NetAudiences, adding: boolean): { changes: SlotList[]; error?: string } {
    const changes: SlotList[] = [];
    for (const slot of targetSlots(store)) {
      const res = listFor(slot, port, name, audiences[slot], adding);
      if (res.error) return { changes, error: res.error };
      if (JSON.stringify(res.list) !== JSON.stringify(listOf(slot))) changes.push({ slot, list: res.list as PublishedPortConfig[] });
    }
    const none = targetSlots(store).every((slot) => audiences[slot] == null);
    if (none) return { changes, error: publishedAfterAdd([], port, name, []).error || ADD_ERROR_NO_NODES };
    return { changes };
  }

  async function publishNets(port: number, name: string, audiences: NetAudiences): Promise<boolean> {
    const plan = planNets(port, name, audiences, true);
    addError.value = plan.error || "";
    return plan.error ? false : writeNets(plan.changes);
  }

  async function editNets(port: number, name: string, audiences: NetAudiences): Promise<boolean> {
    const plan = planNets(port, name, audiences, false);
    editError.value = plan.error || "";
    return plan.error ? false : writeNets(plan.changes, editError);
  }

  async function writeNets(changes: SlotList[], refusal?: Ref<string>): Promise<boolean> {
    if (saving.value) return false;
    saving.value = true;
    try {
      const lists = Object.fromEntries(changes.map((c) => [c.slot, c.list]));
      const failures = await eachSlot(store, changes.map((c) => c.slot), (slot) => saveSlotFields(store, slot, { "ports.published": lists[slot] }, deps));
      const text = failures.map(slotFailureText).join(" ");
      if (failures.length > 0 && refusal) refusal.value = text;
      else saveStatus.value = failures.length === 0 ? { text: CORE_SAVED, cls: "saved" } : { text, cls: "field-error-text" };
      if (failures.length < changes.length) refreshState();
      return failures.length === 0;
    } finally {
      saving.value = false;
    }
  }

  // "Open" and "Close" write the list of the row's own network; a refusal stays at the row.
  async function saveOpened(slot: string, node: string, port: number, list: PortsConfigLists["opened"]): Promise<boolean> {
    if (saving.value) return false;
    saving.value = true;
    const key = netRowKey({ node, port }, isMulti(store) ? slot : undefined);
    try {
      const changes = { "ports.opened": list };
      const res = await saveSlotFields(store, slot, changes, deps);
      const { [key]: _gone, ...rest } = openErrors.value;
      openErrors.value = res.ok ? rest : { ...rest, [key]: fieldErrorText(res.error, changes) };
      if (res.ok) saveStatus.value = { text: CORE_SAVED, cls: "saved" };
      if (res.ok) refreshState();
      return res.ok;
    } finally {
      saving.value = false;
    }
  }

  const openedOf = (slot: string) => configPorts(netConfig(store, slot), "opened");

  function openNetPort(slot: string, node: string, port: number, local?: unknown): Promise<boolean> {
    const taken = new Set(takenLocals(store, { slot, node, port }).keys());
    return saveOpened(slot, node, port, openedAfterOpen(openedOf(slot), node, port, local, taken));
  }

  function closeNetPort(slot: string, node: string, port: number): Promise<boolean> {
    return saveOpened(slot, node, port, openedAfterClose(openedOf(slot), node, port));
  }

  return { saveStatus, addError, editError, saving, openErrors, publish, edit, unpublish, openNetPort, closeNetPort };
}
