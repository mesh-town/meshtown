/* Actions of the "Ports" section: publish, unpublish, "Open" and "Close". The list is edited by the pure
 * functions of `ports-view.ts`, saved by `saveConfigFields`; the result stays with the form until the next edit. */

import { ref } from "vue";
import type { Ref } from "vue";
import type { Store } from "../core/store.ts";
import { notifyChanged } from "../core/bridge.ts";
import { fieldErrorText, saveConfigFields } from "../core/save-config.ts";
import type { SaveDeps } from "../core/save-config.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { CORE_SAVED } from "../core/core-texts.ts";
import type { PortsConfigKey, PortsConfigLists } from "./ports-view.ts";
import {
  configPorts, openedAfterClose, openedAfterOpen, publishedAfterAdd, publishedAfterEdit, publishedAfterRemove,
} from "./ports-view.ts";

// The node snapshot is rebuilt once a second: the second poll after saving catches the fresh one.
const SNAPSHOT_SETTLE_MS = 1200;

/** `deps` — as in `saveConfigFields`, plus `setTimer` to be replaced in tests. */
export type PortsDeps = SaveDeps & { setTimer?: (fn: () => void, ms: number) => unknown };

export interface PortsActions {
  saveStatus: Ref<ExitSaveStatus>;
  addError: Ref<string>;
  editError: Ref<string>;
  saving: Ref<boolean>;
  publish: (portValue: unknown, nameValue: unknown, audience?: "all" | string[]) => Promise<boolean>;
  edit: (port: number, nameValue: unknown, audience: "all" | string[]) => Promise<boolean>;
  unpublish: (port: number) => Promise<boolean>;
  openNetPort: (node: string, port: number, local?: unknown) => Promise<boolean>;
  closeNetPort: (node: string, port: number) => Promise<boolean>;
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
  async function publish(portValue: unknown, nameValue: unknown, audience: "all" | string[] = "all"): Promise<boolean> {
    if (saving.value) return false;
    const res = publishedAfterAdd(current("published"), Number(portValue), String(nameValue ?? "").trim(), audience);
    addError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list);
  }

  // Editing the name and "shared with": a validation error — the text in `editError`, no request.
  async function edit(port: number, nameValue: unknown, audience: "all" | string[]): Promise<boolean> {
    if (saving.value) return false;
    const res = publishedAfterEdit(current("published"), port, String(nameValue ?? "").trim(), audience);
    editError.value = res.error || "";
    if (!res.list) return false;
    return save("published", res.list, editError);
  }

  function unpublish(port: number): Promise<boolean> {
    return save("published", publishedAfterRemove(current("published"), port));
  }

  function openNetPort(node: string, port: number, local?: unknown): Promise<boolean> {
    return save("opened", openedAfterOpen(current("opened"), node, port, local));
  }

  function closeNetPort(node: string, port: number): Promise<boolean> {
    return save("opened", openedAfterClose(current("opened"), node, port));
  }

  return { saveStatus, addError, editError, saving, publish, edit, unpublish, openNetPort, closeNetPort };
}
