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
  const withBrowser: StepId[] = ctx.scope === "browser" && !isPhoneOs(ctx.os) ? ["browser"] : [];
  return ["scope", ...withOs, ...withBrowser, "guide", "exit"];
}

export function wizardPath(ctx: WizardCtx): StepId[] {
  if (ctx.inApp) return ["join", "exit", "connect"];
  if (!ctx.proxyOn || ctx.skipped) return ["join", "scope", "exit"];
  return ["join", ...scopePath(ctx)];
}

/** Whether a step already has an answer: the system and the browser always do (detected), the scope once chosen. */
function answered(ctx: WizardCtx, id: StepId): boolean {
  if (id === "scope") return ctx.proxyOn && ctx.scope !== null;
  return id === "os" || id === "browser";
}

/** The first step from `from` on that still wants an answer: the rows already answered do not stop the cursor. */
export function settleStep(ctx: WizardCtx, from: StepId): StepId {
  const path = wizardPath(ctx);
  const rest = path.slice(Math.max(path.indexOf(from), 0));
  return rest.find((id) => !answered(ctx, id)) ?? from;
}

export function startStep(ctx: WizardCtx, mark: string | null): StepId {
  if (!ctx.hasNet) return "join";
  if (ctx.inApp || mark === "exit") return "exit";
  return settleStep(ctx, wizardPath(ctx)[1]);
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
  if (cur === "scope") return !ctx.proxyOn ? "next" : ctx.scope === null ? "skip" : null;
  return cur === "guide" ? "next" : null;
}

export function markOnEnter(step: StepId): SetupMark | null {
  return step === "exit" ? "exit" : null;
}

export type ScreenId = "choices" | "join" | "exit" | "connect";

/** The screen a step lives on: the questions and the instruction share one page, the other steps have their own. */
export function screenOf(step: StepId): ScreenId {
  return step === "scope" || step === "os" || step === "browser" || step === "guide" ? "choices" : step;
}

/** The rows of the questions page: the choice steps of the path from the first up to the cursor. */
export function visibleRows(ctx: WizardCtx, cursor: StepId): StepId[] {
  const rows = wizardPath(ctx).filter((id) => screenOf(id) === "choices");
  return screenOf(cursor) === "choices" ? rows.slice(0, rows.indexOf(cursor) + 1) : rows;
}

/** Where the cursor lands when the answers changed the path and its step is gone: the next step it still has. */
export function reanchor(before: StepId[], after: StepId[], cur: StepId): StepId {
  const rest = before.slice(Math.max(before.indexOf(cur), 0));
  return rest.find((id) => after.includes(id)) ?? after[after.length - 1];
}

export function joinedLineOn(ctx: WizardCtx, cur: StepId): boolean {
  return ctx.hasNet && screenOf(wizardPath(ctx)[1]) === screenOf(cur);
}

export type StripState = "done" | "current" | "open" | "locked";
export interface StripStep { id: StepId; state: StripState }

function stripState(ctx: WizardCtx, id: StepId, i: number, at: number, visited: ReadonlySet<StepId>): StripState {
  if (i === at) return "current";
  if (id === "join" && ctx.hasNet) return "done";
  if (i < at) return "done";
  return visited.has(id) ? "open" : "locked";
}

/** The clickable strip over the path: what each step is now (passed, current, seen before, not reached yet). */
export function stripSteps(ctx: WizardCtx, cur: StepId, visited: ReadonlySet<StepId>): StripStep[] {
  const path = wizardPath(ctx);
  const at = path.indexOf(cur);
  return path.map((id, i) => ({ id, state: stripState(ctx, id, i, at, visited) }));
}
