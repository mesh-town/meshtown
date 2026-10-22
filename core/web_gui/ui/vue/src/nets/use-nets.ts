// Network selection block: the checkbox draft, saving via `saveConfigFields`, "restart needed" and the restart itself.
import { computed, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import { errReason } from "../common/words.ts";
import { api } from "../core/api.ts";
import type { ApiErrorLike } from "../core/api.ts";
import { fieldErrorText, saveConfigFields } from "../core/save-config.ts";
import type { ConfigChanges } from "../core/save-config.ts";
import type { Store } from "../core/store.ts";
import { ifaceTitle } from "./iface-title.ts";
import { BADGE_AUTO_CLOSED, BADGE_GONE, BADGE_HERE, BADGE_NOT_CHECKED, RESTART_FAILED } from "./nets-labels.ts";
import { AUTO_PAYLOAD, draftNames, isDirty, isRouter, networksOf, openBySaved, savePayload, viaIface } from "./nets-view.ts";
import type { NetsInput } from "./nets-view.ts";

export type RestartPhase = "idle" | "busy" | "done";
const PENDING_FIELDS = ["socks.ifaces", "socks.access"];

export interface NetRow {
  name: string;
  title: string;
  hint: string;
  code: string;
  checked: boolean;
  locked: boolean;
  badge: string;
  warn: boolean;
}

export interface NetsApi {
  rows: ComputedRef<NetRow[]>;
  nodeIsRouter: ComputedRef<boolean>;
  hasSavedNetList: ComputedRef<boolean>;
  dirty: ComputedRef<boolean>;
  canSave: ComputedRef<boolean>;
  empty: ComputedRef<boolean>;
  busy: Ref<boolean>;
  error: Ref<string>;
  hereNote: Ref<boolean>;
  pending: ComputedRef<boolean>;
  phase: ComputedRef<RestartPhase>;
  showSaved: ComputedRef<boolean>;
  toggle(name: string, on: boolean): void;
  lockedClick(ev: Event): void;
  cancel(): void;
  save(): Promise<boolean>;
  setAuto(): Promise<boolean>;
  writeFields(changes: ConfigChanges): Promise<boolean>;
  restart(): Promise<void>;
}

export function useNets(store: Store): NetsApi {
  const edited = ref<string[] | null>(null);
  const busy = ref(false);
  const error = ref("");
  const hereNote = ref(false);
  const restartRaw = ref<RestartPhase>("idle");
  const state = computed<NetsInput>(() => store.state ?? {});
  const saved = computed<string[]>(() => store.config?.socks?.ifaces ?? []);
  const nodeIsRouter = computed(() => isRouter(state.value));
  const hasSavedNetList = computed(() => saved.value.length > 0);
  const names = computed(() => edited.value ?? draftNames(state.value, saved.value));
  const dirty = computed(() => isDirty(names.value, state.value, saved.value));
  const empty = computed(() => names.value.length === 0);
  const canSave = computed(() => dirty.value && !empty.value && !busy.value);
  const pending = computed(() => (store.configMeta?.restart_pending ?? []).some((p) => PENDING_FIELDS.includes(p)));
  const phase = computed<RestartPhase>(() => (restartRaw.value === "busy" && !pending.value ? "done" : restartRaw.value));
  const showSaved = computed(() => pending.value || phase.value !== "idle");

  const rows = computed<NetRow[]>(() => {
    const all = networksOf(state.value);
    return all.map((n) => {
      const t = ifaceTitle(n, nodeIsRouter.value, all);
      const checked = names.value.includes(n.name);
      const locked = n.name === viaIface(state.value);
      const provider = nodeIsRouter.value && n.default_route;
      const badge = rowBadge({ locked, gone: n.present === false, checked, autoClosed: provider && !hasSavedNetList.value });
      const warn = provider && checked && !openBySaved(n, saved.value, nodeIsRouter.value);
      return { name: n.name, title: t.title, hint: t.hint, code: t.code, checked, locked, badge, warn };
    });
  });

  function toggle(name: string, on: boolean): void {
    const base = names.value.filter((n) => n !== name);
    edited.value = on ? networksOf(state.value).map((n) => n.name).filter((n) => n === name || base.includes(n)) : base;
  }
  function lockedClick(ev: Event): void {
    ev.preventDefault();
    hereNote.value = true;
  }
  function cancel(): void {
    edited.value = null;
    hereNote.value = false;
    error.value = "";
  }
  async function writeFields(changes: ConfigChanges): Promise<boolean> {
    busy.value = true;
    error.value = "";
    const res = await saveConfigFields(store, changes);
    busy.value = false;
    if (!res.ok) error.value = fieldErrorText(res.error, changes);
    if (res.ok) edited.value = null;
    return res.ok;
  }
  const save = (): Promise<boolean> => writeFields(savePayload(names.value));
  const setAuto = (): Promise<boolean> => writeFields(AUTO_PAYLOAD);
  async function restart(): Promise<void> {
    restartRaw.value = "busy";
    const failed = await api("POST", "/api/actions/restart", {}).then(() => null, (err: unknown) => err as ApiErrorLike);
    if (failed) restartRaw.value = "idle";
    if (failed) error.value = RESTART_FAILED + errReason(failed);
  }

  return { rows, nodeIsRouter, hasSavedNetList, dirty, canSave, empty, busy, error, hereNote, pending, phase, showSaved, toggle, lockedClick, cancel, save, setAuto, writeFields, restart };
}

function rowBadge(r: { locked: boolean; gone: boolean; checked: boolean; autoClosed: boolean }): string {
  if (r.locked) return BADGE_HERE;
  if (r.gone) return BADGE_GONE;
  if (r.checked) return "";
  return r.autoClosed ? BADGE_AUTO_CLOSED : BADGE_NOT_CHECKED;
}
