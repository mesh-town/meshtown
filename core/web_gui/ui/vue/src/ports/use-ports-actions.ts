/* Actions of the "Shared services (ports)" section: publish, unpublish, "Open" and "Close". The list is edited by the pure
 * functions of `ports-view.ts`, saved by `saveConfigFields`; the result stays with the form until the next edit. */

import { ref } from "vue";
import type { Ref } from "vue";
import type { Store } from "../core/store.ts";
import { notifyChanged } from "../core/bridge.ts";
import { fieldErrorText, saveConfigFields, saveSlotFields } from "../core/save-config.ts";
import { netConfig } from "../core/slots.ts";
import type { SaveDeps } from "../core/save-config.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { CORE_SAVED } from "../core/core-texts.ts";
import type { PortsConfigKey, PortsConfigLists } from "./ports-view.ts";
import {
  configPorts, netRowKey, openedAfterClose, openedAfterOpen, publishedAfterAdd, publishedAfterEdit, publishedAfterRemove, takenLocals,
} from "./ports-view.ts";
import type { PortProto } from "../core/api-types.ts";

// The node snapshot is rebuilt once a second: the second poll after saving catches the fresh one.
const SNAPSHOT_SETTLE_MS = 1200;

/** `deps` — as in `saveConfigFields`, plus `setTimer` to be replaced in tests. */
type PortsDeps = SaveDeps & { setTimer?: (fn: () => void, ms: number) => unknown };

/** "All members" or the ids of the chosen members. */
export type PortAudience = "all" | string[];

interface PortsActions {
  saveStatus: Ref<ExitSaveStatus>;
  addError: Ref<string>;
  editError: Ref<string>;
  saving: Ref<boolean>;
  /** Refusals of "Open" and "Close" by the row key (`netRowKey`). */
  openErrors: Ref<Record<string, string>>;
  publish: (portValue: unknown, nameValue: unknown, audience?: PortAudience, proto?: PortProto) => Promise<boolean>;
  edit: (port: number, nameValue: unknown, audience: PortAudience, proto?: PortProto) => Promise<boolean>;
  unpublish: (port: number, proto?: PortProto) => Promise<boolean>;
  openNetPort: (slot: string, node: string, port: number, local?: unknown, proto?: PortProto) => Promise<boolean>;
  closeNetPort: (slot: string, node: string, port: number, proto?: PortProto) => Promise<boolean>;
}

function defaultBusTarget(): EventTarget | undefined {
  return typeof document === "undefined" ? undefined : document;
}

// `slot` — the network the lists belong to (read when an action starts); `deps` — as in `saveConfigFields`
// (`{ api, busTarget }`); in tests a fake `api`.
export function usePortsActions(store: Store, slot: () => string, deps?: PortsDeps): PortsActions {
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
    return configPorts(netConfig(store, slot()), key);
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
    const net = slot();
    const res = net === "main" ? await saveConfigFields(store, changes, deps) : await saveSlotFields(store, net, changes, deps);
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
  async function publish(portValue: unknown, nameValue: unknown, audience: PortAudience = "all", proto: PortProto = "tcp"): Promise<boolean> {
    if (saving.value) return false;
    const port = Number(portValue);
    const name = String(nameValue ?? "").trim();
    const res = publishedAfterAdd(current("published"), port, name, audience, proto);
    addError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list);
  }

  // Editing the name and "shared with": a validation error — the text in `editError`, no request.
  async function edit(port: number, nameValue: unknown, audience: PortAudience, proto: PortProto = "tcp"): Promise<boolean> {
    if (saving.value) return false;
    const name = String(nameValue ?? "").trim();
    const res = publishedAfterEdit(current("published"), port, name, audience, proto);
    editError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list, editError);
  }

  function unpublish(port: number, proto: PortProto = "tcp"): Promise<boolean> {
    return save("published", publishedAfterRemove(current("published"), port, proto));
  }

  // "Open" and "Close" write the list of the row's own network; a refusal stays at the row.
  async function saveOpened(slot: string, node: string, port: number, proto: PortProto, list: PortsConfigLists["opened"]): Promise<boolean> {
    if (saving.value) return false;
    saving.value = true;
    const key = netRowKey({ node, port, proto });
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

  function openNetPort(slot: string, node: string, port: number, local?: unknown, proto: PortProto = "tcp"): Promise<boolean> {
    const taken = new Set(takenLocals(store, { slot, node, port }, proto).keys());
    return saveOpened(slot, node, port, proto, openedAfterOpen(openedOf(slot), node, port, local, taken, proto));
  }

  function closeNetPort(slot: string, node: string, port: number, proto: PortProto = "tcp"): Promise<boolean> {
    return saveOpened(slot, node, port, proto, openedAfterClose(openedOf(slot), node, port, proto));
  }

  return { saveStatus, addError, editError, saving, openErrors, publish, edit, unpublish, openNetPort, closeNetPort };
}
