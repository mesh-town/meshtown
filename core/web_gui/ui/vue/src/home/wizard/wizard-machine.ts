// The setup wizard as one state and one transition function: the answers the user gave, the cursor, and pure
// functions that derive everything shown (the path, the pressed tiles, the instruction). No framework imports.

import type { BrowserId, OsId } from "../browser-setup.ts";
import type { LanMode } from "../../lan/nets-view.ts";
import type { ProxyScope } from "../proxy-scope.ts";
import { browserTiles, defaultBrowser, guideFor } from "./guide-content.ts";
import type { Guide } from "./guide-content.ts";
import { nextStep, prevStep, reanchor, screenOf, settleStep, stripSteps, validStep, visibleRows, wizardPath } from "./wizard-steps.ts";
import type { StepId, WizBrowser, WizardCtx } from "./wizard-steps.ts";

/** What the user chose; `null` is "not chosen", the page then shows what it detected. */
export interface WizardAnswers {
  scope: ProxyScope | null;
  os: OsId | null;
  pick: WizBrowser | null;
}

export interface WizardCursor {
  at: StepId;
  skipped: boolean;
  visited: StepId[];
}

export type WizardState = WizardAnswers & WizardCursor;

/** What the wizard reads from outside: the node, the page, the setup mark and the detected system and browser. */
export interface WizardEnv {
  hasNet: boolean;
  inApp: boolean;
  proxyOn: boolean;
  mark: string | null;
  lan: LanMode;
  uaOs: OsId;
  uaBrowser: BrowserId;
}

export type WizardEvent =
  | { kind: "scope"; value: ProxyScope }
  | { kind: "os"; value: OsId }
  | { kind: "browser"; value: WizBrowser }
  | { kind: "next" }
  | { kind: "back" }
  | { kind: "strip"; id: StepId }
  | { kind: "skip" }
  | { kind: "toSystem" };

export function freshCursor(): WizardCursor {
  return { at: "join", skipped: false, visited: [] };
}

/** The system shown: the answer, or the detected one; in "one browser" there is no system row, so the detected one
 * (on a router the system row is always there: it names the device being connected). */
export function shownOs(s: WizardState, env: WizardEnv): OsId {
  if (env.lan === "router") return s.os ?? env.uaOs;
  if (s.scope === "browser") return env.uaOs;
  return s.os ?? env.uaOs;
}

/** The browser shown: the pick if this system has its tile, else the system's default. */
export function shownBrowser(s: WizardState, env: WizardEnv): WizBrowser {
  const os = shownOs(s, env);
  const tiles = browserTiles(os).map(([id]) => id);
  return s.pick !== null && tiles.includes(s.pick) ? s.pick : defaultBrowser(os, env.uaBrowser);
}

export function ctxOf(s: WizardState, env: WizardEnv): WizardCtx {
  return { hasNet: env.hasNet, inApp: env.inApp, proxyOn: env.proxyOn, scope: s.scope, os: shownOs(s, env), skipped: s.skipped, router: env.lan === "router" };
}

export function stepOf(s: WizardState, env: WizardEnv): StepId {
  return validStep(ctxOf(s, env), s.at, env.mark);
}

export function guideOf(s: WizardState, env: WizardEnv): Guide {
  return guideFor(s.scope ?? "system", shownOs(s, env), shownBrowser(s, env), env.lan);
}

function advance(s: WizardState, env: WizardEnv): WizardState {
  const ctx = ctxOf(s, env);
  const next = nextStep(ctx, s.at);
  return next === null ? s : { ...s, at: settleStep(ctx, next) };
}

// An answer in row `row`: the path may change under the cursor; only the row under the cursor opens the next one.
function answer(s: WizardState, env: WizardEnv, row: StepId, patch: Partial<WizardState>): WizardState {
  const changed = { ...s, ...patch };
  const at = reanchor(wizardPath(ctxOf(s, env)), wizardPath(ctxOf(changed, env)), s.at);
  const placed = { ...changed, at };
  return row === s.at ? advance(placed, env) : placed;
}

// A row of the questions page: from another screen the page opens at its last row.
function toRow(s: WizardState, env: WizardEnv, id: StepId): WizardState {
  const ctx = ctxOf(s, env);
  const lastRow = prevStep(ctx, "exit");
  if (screenOf(s.at) !== "choices" && lastRow !== null) return { ...s, at: lastRow };
  return visibleRows(ctx, s.at).includes(id) ? s : { ...s, at: id };
}

function toStrip(s: WizardState, env: WizardEnv, id: StepId): WizardState {
  const target = stripSteps(ctxOf(s, env), s.at, new Set(s.visited)).find((t) => t.id === id);
  const reachable = target?.state === "open" || (target?.state === "done" && id !== "join");
  if (!reachable) return s;
  return screenOf(id) === "choices" ? toRow(s, env, id) : { ...s, at: id };
}

/** The next state; the cursor is resolved first, so a state with a stale cursor behaves as it is shown. */
export function applyEvent(state: WizardState, env: WizardEnv, ev: WizardEvent): WizardState {
  const s = { ...state, at: stepOf(state, env) };
  switch (ev.kind) {
    case "scope": return answer(s, env, "scope", { scope: ev.value, skipped: false });
    case "os": return answer(s, env, "os", { os: ev.value });
    case "browser": return answer(s, env, "browser", { pick: ev.value });
    case "next": return advance(s, env);
    case "back": return { ...s, at: prevStep(ctxOf(s, env), s.at) ?? s.at };
    case "strip": return toStrip(s, env, ev.id);
    case "skip": return { ...s, skipped: true, at: "exit" };
    case "toSystem": return { ...s, scope: "system", skipped: false };
  }
}
