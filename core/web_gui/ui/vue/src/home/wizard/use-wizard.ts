/* The setup wizard state: a cursor over the step path computed from the answers, and the actions that move it. */

import { computed, ref } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../../core/store.ts";
import { detectBrowser, detectOs, inAppPage, proxyView } from "../browser-setup.ts";
import type { OsId } from "../browser-setup.ts";
import { readProxyScope, writeProxyScope } from "../proxy-scope.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { useHomeContext } from "../use-home.ts";
import { defaultBrowser, guideFor, manualBrowser } from "./guide-content.ts";
import type { Guide } from "./guide-content.ts";
import { joinedLineOn, markOnEnter, navKind, nextStep, prevStep, stepProgress, validStep } from "./wizard-steps.ts";
import type { NavKind, StepId, WizBrowser, WizardCtx } from "./wizard-steps.ts";

export interface WizardView {
  step: ComputedRef<StepId>;
  ctx: ComputedRef<WizardCtx>;
  progress: ComputedRef<{ n: number; m: number }>;
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
  skip: () => void;
  toSystem: () => void;
  finish: () => void;
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

  const goTo = (target: StepId | null): void => {
    if (!target) return;
    raw.value = target;
    const mark = markOnEnter(target);
    if (mark) h.setMark(mark);
  };
  const goNext = (): void => goTo(nextStep(ctx.value, step.value));
  const setScope = (s: ProxyScope): void => {
    scopeRef.value = s;
    writeProxyScope(s, storage);
  };

  return {
    step, ctx, os, browser, guide,
    progress: computed(() => stepProgress(ctx.value, step.value)),
    back: computed(() => prevStep(ctx.value, step.value) !== null),
    nav: computed(() => navKind(ctx.value, step.value)),
    scope: computed(() => scopeRef.value),
    joined: computed(() => joinedLineOn(ctx.value, step.value)),
    pickScope: (s) => { setScope(s); skipped.value = false; goNext(); },
    pickOs: (o) => { h.pickOs(o); goNext(); },
    pickBrowser: (b) => { pickedBrowser.value = b; h.pickBrowser(manualBrowser(b)); goNext(); },
    goNext,
    goBack: () => goTo(prevStep(ctx.value, step.value)),
    skip: () => { skipped.value = true; goTo("exit"); },
    toSystem: () => { setScope("system"); skipped.value = false; goTo("os"); },
    finish: () => h.setMark("done"),
  };
}
