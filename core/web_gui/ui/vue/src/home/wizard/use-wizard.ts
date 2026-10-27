/* The setup wizard state: a cursor over the step path computed from the answers, and the actions that move it. */

import { computed, nextTick, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../../core/store.ts";
import { detectBrowser, detectOs, inAppPage, proxyView } from "../browser-setup.ts";
import type { OsId } from "../browser-setup.ts";
import { readProxyScope, writeProxyScope } from "../proxy-scope.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { useHomeContext } from "../use-home.ts";
import { defaultBrowser, guideFor, manualBrowser } from "./guide-content.ts";
import type { Guide } from "./guide-content.ts";
import { joinedLineOn, markOnEnter, navKind, nextStep, prevStep, reanchor, screenOf, settleStep, stepProgress, stripSteps, validStep, visibleRows, wizardPath } from "./wizard-steps.ts";
import type { NavKind, ScreenId, StepId, StripStep, WizBrowser, WizardCtx } from "./wizard-steps.ts";

export interface WizardView {
  step: ComputedRef<StepId>;
  screen: ComputedRef<ScreenId>;
  rows: ComputedRef<StepId[]>;
  ctx: ComputedRef<WizardCtx>;
  progress: ComputedRef<{ n: number; m: number }>;
  strip: ComputedRef<StripStep[]>;
  back: ComputedRef<boolean>;
  nav: ComputedRef<NavKind | null>;
  scope: ComputedRef<ProxyScope | null>;
  os: ComputedRef<OsId>;
  browser: ComputedRef<WizBrowser>;
  guide: ComputedRef<Guide>;
  joined: ComputedRef<boolean>;
  pickScope: (s: ProxyScope) => void;
  pickOs: (o: OsId) => void;
  pickBrowser: (b: WizBrowser) => void;
  goNext: () => void;
  goBack: () => void;
  goStep: (id: StepId) => void;
  skip: () => void;
  toSystem: () => void;
  finish: () => void;
}

/** Moves the focus to an element once the DOM has the new screen, and brings it into view. */
function focusById(id: string): void {
  const el = document.getElementById(id);
  el?.focus({ preventScroll: true });
  el?.scrollIntoView?.({ block: "nearest", behavior: "smooth" });
}

const userAgent = (): string => (typeof navigator === "undefined" ? "" : navigator.userAgent || "");

export function useWizard(store: Store): WizardView {
  const { form, hasNet, setupMark, ...h } = useHomeContext(store);
  const storage = typeof localStorage === "undefined" ? null : localStorage;
  const inApp = inAppPage();
  const raw = ref<StepId>("join");
  const scopeRef = ref<ProxyScope | null>(readProxyScope(storage));
  const skipped = ref(false);
  const pickedBrowser = ref<WizBrowser | null>(null);

  const os = computed<OsId>(() => form.browser.os ?? detectOs(userAgent()));
  const ctx = computed<WizardCtx>(() => ({
    hasNet: hasNet.value, inApp, proxyOn: proxyView(store.state).on, scope: scopeRef.value, os: os.value, skipped: skipped.value,
  }));
  const step = computed(() => validStep(ctx.value, raw.value, setupMark.value));
  const browser = computed<WizBrowser>(() => pickedBrowser.value ?? defaultBrowser(os.value, detectBrowser(userAgent())));
  const guide = computed(() => guideFor(scopeRef.value ?? "system", os.value, browser.value));

  const visited = ref<Set<StepId>>(new Set());
  watch(step, (s) => { visited.value = new Set(visited.value).add(s); }, { immediate: true });
  const strip = computed(() => stripSteps(ctx.value, step.value, visited.value));

  // After a step change the focus goes to the heading of the new row (or of the new screen); not on mount.
  let pendingFocus: StepId | null = null;
  watch(step, (s) => {
    const id = pendingFocus ?? s;
    pendingFocus = null;
    focusById(screenOf(id) === "choices" ? `wizard-q-${id}` : "wizard-q");
  }, { flush: "post" });

  const focusSoon = (id: StepId): void => { void nextTick(() => focusById(`wizard-q-${id}`)); };

  const goTo = (target: StepId | null): void => {
    if (!target) return;
    raw.value = target;
    const mark = markOnEnter(target);
    if (mark) h.setMark(mark);
  };
  const goNext = (): void => {
    const next = nextStep(ctx.value, step.value);
    goTo(next && settleStep(ctx.value, next));
  };
  // Records an answer in row `row`: only the row under the cursor opens the next one; the path may change under the cursor.
  const answer = (row: StepId, apply: () => void): void => {
    const before = wizardPath(ctx.value);
    const cur = step.value;
    apply();
    const kept = reanchor(before, wizardPath(ctx.value), cur);
    raw.value = kept;
    if (row === cur) goNext();
  };
  const setScope = (s: ProxyScope): void => {
    scopeRef.value = s;
    writeProxyScope(s, storage);
  };

  // A row of the questions page: reached from the exit screen the page opens at its last row.
  const goRow = (id: StepId): void => {
    const lastRow = prevStep(ctx.value, "exit");
    if (screenOf(step.value) !== "choices" && lastRow) {
      pendingFocus = id;
      goTo(lastRow);
      return;
    }
    if (!visibleRows(ctx.value, step.value).includes(id)) goTo(id);
    focusById(`wizard-q-${id}`);
  };

  const goStep = (id: StepId): void => {
    const target = strip.value.find((t) => t.id === id);
    const reachable = target?.state === "open" || (target?.state === "done" && id !== "join");
    if (reachable && screenOf(id) === "choices") goRow(id);
    else if (reachable) goTo(id);
  };

  return {
    step, strip, goStep, rows: computed(() => visibleRows(ctx.value, step.value)), screen: computed(() => screenOf(step.value)), ctx, os, browser, guide,
    progress: computed(() => stepProgress(ctx.value, step.value)),
    back: computed(() => screenOf(step.value) !== "choices" && prevStep(ctx.value, step.value) !== null),
    nav: computed(() => navKind(ctx.value, step.value)),
    scope: computed(() => scopeRef.value),
    joined: computed(() => joinedLineOn(ctx.value, step.value)),
    pickScope: (s) => answer("scope", () => { setScope(s); skipped.value = false; }),
    pickOs: (o) => answer("os", () => h.pickOs(o)),
    pickBrowser: (b) => answer("browser", () => { pickedBrowser.value = b; h.pickBrowser(manualBrowser(b)); }),
    goNext,
    goBack: () => goTo(prevStep(ctx.value, step.value)),
    skip: () => { skipped.value = true; goTo("exit"); },
    toSystem: () => { setScope("system"); skipped.value = false; focusSoon("os"); },
    finish: () => h.setMark("done"),
  };
}
