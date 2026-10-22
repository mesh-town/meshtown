/* Core of the "Site checks" section actions without Vue (tested in node): add,
 * recheck, remove, "Check all". `ui` is the state object (reactive in the section):
 * `busy`, `errors`, `addError`, `checkAllBusy`, `checkAllStatus`; `api(method, path, body)` is the request. */

import type { ApiFn } from "../core/api.ts";
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

export function createDomainActions(ui: DomainUi, api: ApiFn): DomainActions {
  async function rowAction(domain: string, request: () => Promise<unknown>, failMessage: (name: string, err: unknown) => string): Promise<void> {
    if (ui.busy[domain]) return;
    ui.busy[domain] = true;
    delete ui.errors[domain];
    try {
      await request();
    } catch (err) {
      ui.errors[domain] = failMessage(domain, err);
    } finally {
      ui.busy[domain] = false;
    }
  }

  const recheck = (domain: string): Promise<void> =>
    rowAction(domain, () => api("POST", "/api/actions/check-domain", { domain }), recheckFailedText);

  const remove = (domain: string): Promise<void> =>
    rowAction(domain, () => api("DELETE", "/api/domains?name=" + encodeURIComponent(domain)), removeFailedText);

  // Returns true when the site was added: the input field is then cleared.
  async function add(raw: string): Promise<boolean> {
    const domain = raw.trim();
    if (!domain) return false;
    ui.addError = "";
    try {
      await api("POST", "/api/domains", { domain });
      return true;
    } catch (err) {
      ui.addError = addFailedText(err);
      return false;
    }
  }

  async function checkAll(): Promise<void> {
    if (ui.checkAllBusy) return;
    ui.checkAllBusy = true;
    ui.checkAllStatus = "";
    try {
      await api("POST", "/api/actions/check-all", {});
    } catch (err) {
      ui.checkAllStatus = (err as { status?: number } | null)?.status === 409 ? CHECK_ALL_CONFLICT : checkAllFailedText(err);
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
