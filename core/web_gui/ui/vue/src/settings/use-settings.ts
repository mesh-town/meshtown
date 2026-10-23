/* The core wrapper of the "Settings" section for the section: the reactive form, computed views and
 * watchers. The logic is in `settings-actions.ts`, `settings-view.ts`; here only `reactive`/`watch`. */

import { computed, onUnmounted, reactive, watch } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../core/store.ts";
import {
  createSettingsForm, defaultSettingsDeps, disposeForm, editField, onPendingEmpty, reloadFresh, resetDraft,
  restartNode, rotateToken, saveDraft, stopNode, toggleToken, resolveSettingsDeps,
} from "./settings-actions.ts";
import type { RestartOutcome, SaveOutcome, SettingsDeps, SettingsForm } from "./settings-actions.ts";
import type { Field, RawValue } from "./settings-fields.ts";
import { fieldView, fileBanner, restartTargets, saveBarView } from "./settings-view.ts";
import type { FieldView, FileBanner, SaveBarView } from "./settings-view.ts";

export interface Settings {
  form: SettingsForm;
  dirty: ComputedRef<boolean>;
  bar: ComputedRef<SaveBarView>;
  banner: ComputedRef<FileBanner | null>;
  view: (field: Field) => FieldView;
  edit: (path: string, raw: RawValue) => void;
  save: () => Promise<SaveOutcome>;
  overwrite: () => Promise<SaveOutcome>;
  reset: () => void;
  reload: () => void;
  restart: () => Promise<RestartOutcome>;
  stop: () => Promise<boolean>;
  rotate: () => Promise<boolean>;
  toggleToken: () => void;
}

// The "unsaved" label on the tab is `store.settingsDirty` (drawn by `shell/TabsNav.vue`).
export function useSettings(store: Store, deps: Partial<SettingsDeps> = defaultSettingsDeps): Settings {
  const d = resolveSettingsDeps(deps);
  const form = reactive<SettingsForm>(createSettingsForm());
  const dirty = computed(() => Object.keys(form.draft).length > 0);
  const bar = computed(() => saveBarView({
    dirty: dirty.value, meta: store.configMeta, configFile: store.configFile,
    requested: form.restartRequested, busy: form.busy, message: form.message, restartTargets: restartTargets(store),
  }));
  const banner = computed(() => fileBanner(store.configFile, form.hideFileFieldErrors));
  const view = (field: Field): FieldView => fieldView(field, {
    config: store.config, meta: store.configMeta, state: store.state, draft: form.draft, errors: form.errors,
  });

  // A restart is requested but `restart_pending` is already empty (in any order of response and poll) — the request is cleared.
  const restartDone = (): boolean => restartTargets(store).length === 0 && form.restartRequested;
  watch(dirty, (on) => { store.settingsDirty = on; }, { immediate: true });
  watch(restartDone, (done) => done && onPendingEmpty(form));
  onUnmounted(() => {
    store.settingsDirty = false;
    disposeForm(form, d);
  });

  return {
    form, dirty, bar, banner, view,
    edit: (path: string, raw: RawValue) => editField(form, path, raw),
    save: () => saveDraft(form, store, d),
    overwrite: () => (form.conflict ? saveDraft(form, store, d, form.conflict.rev) : Promise.resolve("skipped")),
    reset: () => resetDraft(form),
    reload: () => reloadFresh(form, d),
    restart: () => restartNode(form, d, store),
    stop: () => stopNode(form, d),
    rotate: () => rotateToken(form, d),
    toggleToken: () => toggleToken(form, d),
  };
}
