// The step graph of the setup wizard: the path is computed from the answers, Back/Next are neighbours in it.
// Pure functions, no framework imports.

import type { BrowserId, OsId } from "../browser-setup.ts";
import type { SetupMark } from "../home-actions.ts";
import type { ProxyScope } from "../proxy-scope.ts";

export type StepId = "join" | "scope" | "os" | "browser" | "guide" | "exit" | "connect";
export type WizBrowser = BrowserId | "other";
export type NavKind = "next" | "skip" | "finish" | "done";

export interface WizardCtx {
  hasNet: boolean;
  inApp: boolean;
  proxyOn: boolean;
  scope: ProxyScope | null;
  os: OsId;
  skipped: boolean;
}

export function isPhoneOs(os: OsId): boolean {
  return os === "android" || os === "ios";
}

function scopePath(ctx: WizardCtx): StepId[] {
  const withOs: StepId[] = ctx.scope === "browser" ? [] : ["os"];
  const withBrowser: StepId[] = isPhoneOs(ctx.os) ? [] : ["browser"];
  return ["scope", ...withOs, ...withBrowser, "guide", "exit"];
}

export function wizardPath(ctx: WizardCtx): StepId[] {
  if (ctx.inApp) return ["join", "exit", "connect"];
  if (!ctx.proxyOn || ctx.skipped) return ["join", "scope", "exit"];
  return ["join", ...scopePath(ctx)];
}

export function startStep(ctx: WizardCtx, mark: string | null): StepId {
  if (!ctx.hasNet) return "join";
  if (ctx.inApp || mark === "exit") return "exit";
  return wizardPath(ctx)[1];
}

export function validStep(ctx: WizardCtx, cur: StepId, mark: string | null): StepId {
  const joinedAlready = cur === "join" && ctx.hasNet;
  if (joinedAlready || !wizardPath(ctx).includes(cur)) return startStep(ctx, mark);
  return cur;
}

export function nextStep(ctx: WizardCtx, cur: StepId): StepId | null {
  const path = wizardPath(ctx);
  const i = path.indexOf(cur);
  return i < 0 ? null : path[i + 1] ?? null;
}

export function prevStep(ctx: WizardCtx, cur: StepId): StepId | null {
  const path = wizardPath(ctx);
  const i = path.indexOf(cur);
  const prev = i > 0 ? path[i - 1] : null;
  return prev === "join" && ctx.hasNet ? null : prev;
}

export function stepProgress(ctx: WizardCtx, cur: StepId): { n: number; m: number } {
  const path = wizardPath(ctx);
  return { n: Math.max(path.indexOf(cur), 0) + 1, m: path.length };
}

export function navKind(ctx: WizardCtx, cur: StepId): NavKind | null {
  if (cur === "join") return null;
  if (cur === "connect") return "done";
  if (cur === "exit") return ctx.inApp ? "next" : "finish";
  return cur === "scope" && ctx.proxyOn && ctx.scope === null ? "skip" : "next";
}

export function markOnEnter(step: StepId): SetupMark | null {
  return step === "exit" ? "exit" : null;
}

export function joinedLineOn(ctx: WizardCtx, cur: StepId): boolean {
  return ctx.hasNet && wizardPath(ctx)[1] === cur;
}
