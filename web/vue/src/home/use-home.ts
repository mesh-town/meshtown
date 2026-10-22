/* The Home core wrapper for the section: form, computed views, observers and actions.
 * The logic is in `home-*.js`, `exit-list.ts`, `browser-setup.ts`; here only `reactive`/`computed`/`watch`.
 * The notice after creating a network (`notice`) lives here, not in the join card: when a network appears the card
 * moves to "Settings", while the text stays in "Invite a member". */

import { computed, inject, onUnmounted, reactive, ref, watch } from "vue";
import type { ComputedRef, InjectionKey, Ref } from "vue";
import type { ConfigDocument, NetworkInfo } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import { chosenExitIds, getRememberedExits, markExitAutoPicked, wasExitAutoPicked } from "../common/exits.ts";
import { getPath } from "../settings/settings-fields.ts";
import { isAppUserAgent, proxyState } from "./browser-setup.ts";
import type { BrowserId, OsId, ProxyState, StepValue } from "./browser-setup.ts";
import { exitWarning, memberExits, warnPanel } from "../members/exit-members.ts";
import type { ExitWarning } from "../members/exit-members.ts";
import { isMeExcluded } from "../members/members-ids.ts";
import { autoExitPick, exitCandidates, findExitView, noPeersYet, otherExitSelector } from "./exit-list.ts";
import type { ExitCandidate, FindExitView } from "./exit-list.ts";
import {
  chooseDirect, chooseExit, clearHomeTimers, copyValue, createHomeForm, defaultHomeDeps, findExit, storageOf,
  readSetupMark, saveExitServer, saveName, writeSetupMark,
} from "./home-actions.ts";
import type { ExitSaveResult, HomeDeps, HomeForm, SetupMark } from "./home-actions.ts";
import { activeTabOf, cardOrder, configFieldAccess, HOME_ORDER, homeMode, nodeNameFieldValue, statePanel, wizardStep } from "./home-view.ts";
import type { CardKey, HomeMode, HomeTab, StatePanelView } from "./home-view.ts";
import { ringView } from "./ring-view.ts";
import type { RingView } from "./ring-view.ts";
import { joinProgress } from "./join-progress.ts";
import type { JoinProgress, JoinSent } from "./join-progress.ts";
import { exitServerHint, membersCountText, netStatusText } from "./home-words.ts";

export interface ExitServerView {
  checked: boolean;
  readOnly: boolean;
  cliNote: string;
  hint: string;
}

export interface NameView {
  readOnly: boolean;
  cliNote: string;
  value: string;
}

/** What `useHome` gives to the Home cards. */
export interface HomeContext {
  form: HomeForm;
  setupMark: Ref<string | null>;
  notice: Ref<string>;
  candidates: ComputedRef<ExitCandidate[]>;
  warning: ComputedRef<ExitWarning | null>;
  networkLabel: ComputedRef<string | null>;
  exitServer: ComputedRef<ExitServerView>;
  hasNet: ComputedRef<boolean>;
  searching: ComputedRef<boolean>;
  mode: ComputedRef<HomeMode>;
  wizardStepNumber: ComputedRef<number>;
  homeTab: Ref<HomeTab | null>;
  activeTab: ComputedRef<HomeTab>;
  joinSent: Ref<JoinSent | null>;
  progress: ComputedRef<JoinProgress | null>;
  panel: ComputedRef<StatePanelView>;
  ring: ComputedRef<RingView | null>;
  order: ComputedRef<CardKey[]>;
  findView: ComputedRef<FindExitView>;
  proxy: ComputedRef<ProxyState>;
  members: ComputedRef<string | null>;
  netStatus: ComputedRef<string | null>;
  nameView: ComputedRef<NameView>;
  chooseExit: (idFull: string) => Promise<ExitSaveResult>;
  chooseDirect: () => Promise<ExitSaveResult>;
  findExit: () => Promise<"empty" | "sent" | "failed">;
  saveName: () => Promise<"same" | "saved" | "failed">;
  saveExitServer: (checked: boolean) => Promise<ExitSaveResult>;
  copy: (kind: StepValue) => Promise<"fallback" | "copied">;
  pickBrowser: (id: BrowserId) => void;
  pickOs: (id: OsId) => void;
  toggleBrowser: () => void;
  pickOther: () => void;
  setMark: (mark: SetupMark) => void;
}

// With nobody else in the network the status line stays hidden: the counter already says it.
const hasOthers = (state: Store["state"]): boolean => ((state && state.peers) || []).some((p) => !p.self);

export const HOME_KEY: InjectionKey<HomeContext> = Symbol("meshtown.home");

const netOf = (store: Store): NetworkInfo | null => (store.state && store.state.network) || null;
const netIdOf = (store: Store): string | null => netOf(store)?.id_hex || null;

// While the name is not being edited and has no focus — the field follows the saved value.
function syncName(form: HomeForm, value: string): void {
  const name = form.name;
  if (name.focused || name.draft !== name.original) return;
  name.draft = value;
  name.original = value;
}

// During our PUT the choice is not overwritten — the PUT itself will return or confirm it.
function syncExit(form: HomeForm, value: string): void {
  if (form.exitSavesInFlight) return;
  form.exitChoice = value;
}

// An explicit tab choice lives until reload; "Done" in the setup resets it — "Main" opens.
function useMark(store: Store, deps: HomeDeps, homeTab: Ref<HomeTab | null>): { setupMark: Ref<string | null>; setMark: (mark: SetupMark) => void } {
  const setupMark = ref<string | null>(readSetupMark(deps.storage));
  const reread = () => { setupMark.value = readSetupMark(deps.storage) || setupMark.value; };
  const setMark = (mark: SetupMark): void => {
    writeSetupMark(mark, deps.storage);
    setupMark.value = mark;
    if (mark === "done") homeTab.value = null;
  };
  watch(() => store.state, reread);
  return { setupMark, setMark };
}

function useNotice(store: Store): Ref<string> {
  const notice = ref("");
  watch(() => netIdOf(store), (id, prev) => { if (prev && id !== prev) notice.value = ""; });
  return notice;
}

// Proxy off — an empty string (there are no buttons then).
const proxyCopyText = (proxy: ProxyState, kind: StepValue): string => (proxy.on ? (kind === "lan" ? proxy.lan?.[0]?.addr : kind === "pac" ? proxy.pacUrl : proxy[kind]) ?? "" : "");

const proxyOf = (store: Store, form: HomeForm, deps: HomeDeps): ProxyState => proxyState(store.state, form.browser.choice, deps.userAgent, form.browser.os);

// Inside the phone app, a node that joined a network picks the nearest online exit once, when none was ever chosen.
function useAutoExit(store: Store, deps: HomeDeps, candidates: ComputedRef<ExitCandidate[]>, pick: (id: string) => unknown): void {
  const idle = (): boolean => isAppUserAgent(deps.userAgent) && !!store.config && !!netOf(store) && !wasExitAutoPicked(storageOf(deps));
  const free = (): boolean => chosenExitIds(store).length === 0 && getRememberedExits(storageOf(deps)).length === 0;
  watch(candidates, (list) => {
    const id = idle() && free() ? autoExitPick(list) : null;
    if (id === null) return;
    markExitAutoPicked(storageOf(deps));
    pick(id);
  }, { immediate: true });
}

export function useHome(store: Store, deps: HomeDeps = defaultHomeDeps): HomeContext {
  const form = reactive(createHomeForm());
  const homeTab = ref<HomeTab | null>(null);
  const { setupMark, setMark } = useMark(store, deps, homeTab);
  const mode = computed(() => homeMode(store.state, setupMark.value));
  const notice = useNotice(store);
  const chosen = computed(() => chosenExitIds(store));
  const warning = computed(() => exitWarning(chosen.value, store.members, isMeExcluded(store.state, store.members)));
  const candidates = computed(() => memberExits(exitCandidates(store.state, chosen.value, getRememberedExits(storageOf(deps))), store.members));
  const joinSent = ref<JoinSent | null>(null);
  const progress = computed(() => joinProgress(store.state, joinSent.value, store.absence ?? null));
  const panel = computed(() => warnPanel(statePanel(store.state, store.absence ?? null, progress.value), warning.value));
  const networkLabel = computed(() => netOf(store)?.label || null);
  const exitServer = computed((): ExitServerView => {
    const checked = !!getPath(store.config as ConfigDocument | null, "exit.enabled");
    return { checked, ...configFieldAccess(store.configMeta, "exit.enabled"), hint: exitServerHint(checked, networkLabel.value) };
  });

  useAutoExit(store, deps, candidates, (id) => chooseExit(form, store, id, deps));
  watch(() => chosen.value[0] ?? "", (v) => syncExit(form, v), { immediate: true });
  watch(() => nodeNameFieldValue(store.config, store.state), (v) => syncName(form, v), { immediate: true });
  onUnmounted(() => clearHomeTimers(form, deps));

  return {
    form, setupMark, notice, candidates, warning, networkLabel, exitServer,
    hasNet: computed(() => !!netOf(store)),
    searching: computed(() => noPeersYet(store.state)),
    mode,
    wizardStepNumber: computed(() => wizardStep(store.state, setupMark.value)),
    homeTab,
    activeTab: computed(() => activeTabOf(homeTab.value, mode.value)),
    joinSent, progress, panel,
    ring: computed(() => (panel.value.busy ? null : ringView(store.state, panel.value.level))),
    order: computed(() => cardOrder(store.state, HOME_ORDER)),
    findView: computed(() => findExitView(store.state, form.findDomain)),
    proxy: computed(() => proxyOf(store, form, deps)),
    members: computed(() => membersCountText(store.state)),
    netStatus: computed(() => (hasOthers(store.state) ? netStatusText(store.state) : null)),
    nameView: computed(() => ({ ...configFieldAccess(store.configMeta, "node.name"), value: nodeNameFieldValue(store.config, store.state) })),
    chooseExit: (idFull: string) => chooseExit(form, store, idFull, deps),
    chooseDirect: () => chooseDirect(form, store, deps),
    findExit: () => findExit(form, deps),
    saveName: () => saveName(form, store, deps),
    saveExitServer: (checked: boolean) => saveExitServer(form, store, checked, deps),
    copy: (kind: StepValue) => copyValue(form, kind, proxyCopyText(proxyOf(store, form, deps), kind), deps),
    pickBrowser: (id: BrowserId) => { form.browser.choice = id; },
    pickOs: (id: OsId) => { form.browser.os = id; },
    toggleBrowser: () => { form.browser.open = !form.browser.open; },
    pickOther: () => deps.focus(otherExitSelector(candidates.value)),
    setMark,
  };
}

// Cards take the section's shared `useHome`; those mounted on their own create their own.
export function useHomeContext(store: Store): HomeContext {
  return inject(HOME_KEY, null) || useHome(store);
}
