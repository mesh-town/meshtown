/* The setup wizard on the page: the answers and the cursor live in the section's `form`; `applyEvent` computes every
 * move, this file feeds it the environment and carries out what a move needs (the answers, the mark, the focus). */

import { computed, nextTick, watch } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../../core/store.ts";
import { detectBrowser, detectOs, inAppPage, proxyView } from "../browser-setup.ts";
import type { OsId } from "../browser-setup.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { useHomeContext } from "../use-home.ts";
import type { Guide } from "./guide-content.ts";
import { applyEvent, ctxOf, guideOf, shownBrowser, shownOs, stepOf } from "./wizard-machine.ts";
import type { WizardEnv, WizardEvent, WizardState } from "./wizard-machine.ts";
import { joinedLineOn, markOnEnter, navKind, prevStep, screenOf, stepProgress, stripSteps, visibleRows } from "./wizard-steps.ts";
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
  const inApp = inAppPage();
  const env = computed<WizardEnv>(() => ({
    hasNet: hasNet.value, inApp, proxyOn: proxyView(store.state).on, mark: setupMark.value,
    uaOs: detectOs(userAgent()), uaBrowser: detectBrowser(userAgent()),
  }));
  const state = computed<WizardState>(() => ({ scope: form.browser.scope, os: form.browser.os, pick: form.browser.pick, ...form.wizard }));
  const ctx = computed<WizardCtx>(() => ctxOf(state.value, env.value));
  const step = computed(() => stepOf(state.value, env.value));
  const strip = computed(() => stripSteps(ctx.value, step.value, new Set(form.wizard.visited)));

  watch(step, (s) => { if (!form.wizard.visited.includes(s)) form.wizard.visited = [...form.wizard.visited, s]; }, { immediate: true });

  // After a step change the focus goes to the heading of the new row (or of the new screen); not on mount.
  let pendingFocus: StepId | null = null;
  watch(step, (s) => {
    const id = pendingFocus ?? s;
    pendingFocus = null;
    focusById(screenOf(id) === "choices" ? `wizard-q-${id}` : "wizard-q");
  }, { flush: "post" });

  const focusSoon = (id: StepId): void => { void nextTick(() => focusById(`wizard-q-${id}`)); };

  // The focus a strip click asks for: a row of the questions page, reached from another screen once the page is open.
  const focusStrip = (id: StepId): void => {
    if (screenOf(id) !== "choices" || !strip.value.some((t) => t.id === id && t.state !== "locked" && t.state !== "current")) return;
    if (screenOf(step.value) === "choices") focusById(`wizard-q-${id}`);
    else pendingFocus = id;
  };

  const commitAnswers = (before: WizardState, after: WizardState): void => {
    if (after.scope !== before.scope && after.scope) h.pickScope(after.scope);
    if (after.os !== before.os && after.os) h.pickOs(after.os);
    if (after.pick !== before.pick && after.pick) h.pickWizBrowser(after.pick);
  };

  const dispatch = (ev: WizardEvent): void => {
    const before = state.value;
    const from = step.value;
    const after = applyEvent(before, env.value, ev);
    if (ev.kind === "strip") focusStrip(ev.id);
    commitAnswers(before, after);
    form.wizard = { at: after.at, skipped: after.skipped, visited: after.visited };
    const mark = from === step.value ? null : markOnEnter(step.value);
    if (mark) h.setMark(mark);
    if (ev.kind === "toSystem") focusSoon("os");
  };

  return {
    step, strip, ctx, goStep: (id) => dispatch({ kind: "strip", id }),
    rows: computed(() => visibleRows(ctx.value, step.value)), screen: computed(() => screenOf(step.value)),
    os: computed(() => shownOs(state.value, env.value)),
    browser: computed(() => shownBrowser(state.value, env.value)),
    guide: computed(() => guideOf(state.value, env.value)),
    progress: computed(() => stepProgress(ctx.value, step.value)),
    back: computed(() => screenOf(step.value) !== "choices" && prevStep(ctx.value, step.value) !== null),
    nav: computed(() => navKind(ctx.value, step.value)),
    scope: computed(() => form.browser.scope),
    joined: computed(() => joinedLineOn(ctx.value, step.value)),
    pickScope: (value) => dispatch({ kind: "scope", value }),
    pickOs: (value) => dispatch({ kind: "os", value }),
    pickBrowser: (value) => dispatch({ kind: "browser", value }),
    goNext: () => dispatch({ kind: "next" }),
    goBack: () => dispatch({ kind: "back" }),
    skip: () => dispatch({ kind: "skip" }),
    toSystem: () => dispatch({ kind: "toSystem" }),
    finish: () => h.setMark("done"),
  };
}
