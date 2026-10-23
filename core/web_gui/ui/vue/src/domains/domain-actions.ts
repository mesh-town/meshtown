/* Core of the "Site checks" section actions without Vue (tested in node): add,
 * recheck, remove, "Check all". `ui` is the state object (reactive in the section):
 * `busy`, `errors`, `addError`, `checkAllBusy`, `checkAllStatus`; `api(method, path, body)` is the request. */

import type { ApiFn } from "../core/api.ts";
import type { NetBrief } from "../core/api-types-overview.ts";
import { asResult, eachSlot, slotFailureText } from "../core/save-config.ts";
import type { SlotFailure } from "../core/save-config.ts";
import { slotPath } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import {
  CHECK_ALL_CONFLICT, addFailedText, checkAllFailedText, recheckFailedText, removeFailedText,
} from "./domains-view.ts";

/** Section state: busy flags and error texts per site, the add error, the "Check all" status. */
export interface DomainUi {
  busy: Record<string, boolean>;
  errors: Record<string, string>;
  addError: string;
  checkAllBusy: boolean;
  checkAllStatus: string;
}

export interface DomainActions {
  recheck: (domain: string) => Promise<void>;
  remove: (domain: string) => Promise<void>;
  add: (raw: string) => Promise<boolean>;
  checkAll: () => Promise<void>;
  onRowsBusy: (now: boolean, was: boolean | undefined) => void;
}

export function emptyUi(): DomainUi {
  return { busy: {}, errors: {}, addError: "", checkAllBusy: false, checkAllStatus: "" };
}

/** Which networks an action goes to. `holding(name)` — networks whose list has the site. */
export interface DomainTargets {
  all(): string[];
  holding(name: string): string[];
  multi(): boolean;
  nets?(): NetBrief[];
}

const MAIN_ONLY: DomainTargets = { all: () => ["main"], holding: () => ["main"], multi: () => false };

const isConflict = (err: unknown): boolean => (err as { status?: number } | null)?.status === 409;

export function createDomainActions(ui: DomainUi, api: ApiFn, t: DomainTargets = MAIN_ONLY): DomainActions {
  // Writes go one network at a time; a failure in one does not stop the others.
  const run = (slots: string[], write: (slot: string) => Promise<unknown>): Promise<SlotFailure[]> =>
    eachSlot({ nets: t.nets?.() ?? [] } as Store, slots, (slot) => asResult(write(slot).then(() => undefined)));

  // One network: the node's own error text; several: every failure with its network label.
  const failureText = (failures: SlotFailure[], single: (err: unknown) => string): string =>
    t.multi() ? failures.map(slotFailureText).join(" ") : single(failures[0].error);

  async function rowAction(domain: string, request: (slot: string) => Promise<unknown>, failMessage: (name: string, err: unknown) => string): Promise<void> {
    if (ui.busy[domain]) return;
    ui.busy[domain] = true;
    delete ui.errors[domain];
    const slots = t.multi() ? t.holding(domain) : t.all();
    const failures = await run(slots, request).finally(() => { ui.busy[domain] = false; });
    if (failures.length > 0) ui.errors[domain] = failureText(failures, (err) => failMessage(domain, err));
  }

  const recheck = (domain: string): Promise<void> =>
    rowAction(domain, (slot) => api("POST", slotPath(slot, "actions/check-domain"), { domain }), recheckFailedText);

  const remove = (domain: string): Promise<void> =>
    rowAction(domain, (slot) => api("DELETE", slotPath(slot, "domains") + "?name=" + encodeURIComponent(domain)), removeFailedText);

  // Returns true when the site was added somewhere (or is already everywhere): the input field is then cleared.
  async function add(raw: string): Promise<boolean> {
    const domain = raw.trim();
    if (!domain) return false;
    ui.addError = "";
    const have = t.holding(domain);
    const slots = t.multi() ? t.all().filter((s) => !have.includes(s)) : t.all();
    const failures = await run(slots, (slot) => api("POST", slotPath(slot, "domains"), { domain }));
    if (failures.length > 0) ui.addError = failureText(failures, addFailedText);
    return failures.length < slots.length || (slots.length === 0 && have.length > 0);
  }

  // A busy node (409) is no failure when another network took the request; all busy — a conflict.
  async function checkAllSlots(slots: string[]): Promise<string> {
    let busy = 0;
    const failures = await run(slots, (slot) =>
      api("POST", slotPath(slot, "actions/check-all"), {}).catch((err: unknown) => {
        if (!isConflict(err)) throw err;
        busy += 1;
      }));
    if (failures.length > 0) return failureText(failures, checkAllFailedText);
    return busy === slots.length ? CHECK_ALL_CONFLICT : "";
  }

  async function checkAll(): Promise<void> {
    if (ui.checkAllBusy) return;
    ui.checkAllBusy = true;
    ui.checkAllStatus = "";
    try {
      ui.checkAllStatus = await checkAllSlots(t.all());
    } finally {
      ui.checkAllBusy = false;
    }
  }

  // The "Check all" status is cleared when the rows stop being checked — not on every
  // redraw, otherwise the text would vanish on the next poll.
  function onRowsBusy(now: boolean, was: boolean | undefined): void {
    if (was && !now) ui.checkAllStatus = "";
  }

  return { recheck, remove, add, checkAll, onRowsBusy };
}
