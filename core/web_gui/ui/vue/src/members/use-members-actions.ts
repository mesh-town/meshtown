/* Actions of the Members tab: ask in the dialog, send to `POST /api/members/<action>`, keep the answer in
 * `store.members`, wake the poller, and produce the line under the list. Does not draw anything. */

import { nextTick, ref } from "vue";
import type { Ref } from "vue";
import { api as coreApi } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import { notifyChanged } from "../core/bridge.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { dropSlot, putSlotMembers } from "../core/overview.ts";
import { membersOf, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { chosenExitIds } from "../common/exits.ts";
import { navigateTo } from "../common/navigate.ts";
import { dialogSpec } from "./dialog-specs.ts";
import type { DialogKind, DialogSpec } from "./dialog-specs.ts";
import { leaveNet, memberErrorText, postMemberAction } from "./members-api.ts";
import { admittedStatus, blockedStatus, confirmedStatus, leftNetworkStatus, removalProposedStatus, removedStatus, unadmittedStatus, withdrawnStatus } from "./members-texts.ts";
import type { MemberAction, MembersReply } from "./members-types.ts";
import { displayNames, meId, memberById } from "./members-view.ts";

/** What `ConfirmDialog` exposes outward through `defineExpose`. */
export interface DialogHandle {
  ask: (spec: DialogSpec) => Promise<boolean>;
  reason?: () => string;
}

export interface MembersActionsDeps {
  api?: ApiFn;
  busTarget?: EventTarget;
}

export interface MembersActions {
  status: Ref<ExitSaveStatus>;
  dialog: Ref<DialogHandle | null>;
  ask: (kind: DialogKind, idFull: string) => Promise<void>;
  run: (action: MemberAction, idFull: string, reason?: string) => Promise<void>;
  pick: (key: string, idFull: string) => Promise<void>;
  joinAnother: () => Promise<void>;
}

const ACTION_OF: Record<DialogKind, MemberAction> = { remove: "exclude", "remove-two": "exclude", confirm: "exclude", admit: "admit", block: "block", leave: "leave" };
const INFO = (text: string): ExitSaveStatus => ({ text, cls: "" });
const FAILED = (text: string): ExitSaveStatus => ({ text, cls: "field-error-text" });

// The node writes its own sentence for a refused action; without one the text comes from the error code.
function failureText(err: unknown): string {
  const said = (err as { body?: { message?: unknown } } | null)?.body?.message;
  return typeof said === "string" && said.trim() ? said : memberErrorText(err);
}

export function useMembersActions(store: Store, slot: () => string = () => store.lead, deps: MembersActionsDeps = {}): MembersActions {
  const status = ref<ExitSaveStatus>({ text: "", cls: "" });
  const dialog = ref<DialogHandle | null>(null);
  const call: ApiFn = deps.api || coreApi;
  const net = (): string => stateOf(store, slot())?.network?.label || "";
  const nameOf = (idFull: string): string => displayNames(membersOf(store, slot()))[idFull] || idFull.slice(0, 8);

  function context(kind: DialogKind, idFull: string) {
    const row = memberById(membersOf(store, slot()), idFull);
    const by = row && row.vote ? nameOf(row.vote.proposed_by) : undefined;
    const label = row && row.admission ? row.admission.label : null;
    return { name: nameOf(idFull), by, label, reason: row && row.vote ? row.vote.reason : null, net: net(), isMyExit: chosenExitIds(store).includes(idFull) };
  }

  function wake(): void {
    const target = deps.busTarget || (typeof document === "undefined" ? undefined : document);
    if (target) notifyChanged(target, "state");
  }

  // The line under the list after an accepted action.
  function doneText(action: MemberAction, idFull: string, reply: MembersReply, netName: string): string {
    if (action === "unblock") return "";
    if (action === "unexclude") return withdrawnStatus(nameOf(idFull), memberById(reply, idFull)?.blocked_by_me === true);
    if (action === "block") return blockedStatus(nameOf(idFull));
    if (action === "unadmit") return unadmittedStatus(nameOf(idFull));
    if (action === "admit") return memberById(reply, idFull)?.status === "member" ? admittedStatus(nameOf(idFull)) : confirmedStatus(nameOf(idFull));
    const row = memberById(reply, idFull);
    return row && row.status === "excluded" ? removedStatus(nameOf(idFull)) : removalProposedStatus(nameOf(idFull));
  }

  // Leaving: a left network disappears from the page; a refused leave only refreshes the network list.
  async function leave(): Promise<void> {
    const from = slot();
    const netName = net();
    const reply = await leaveNet(call, from);
    if (reply.left) {
      const sliced = store.slots[from] !== undefined;
      dropSlot(store, from, reply.nets);
      if (!sliced && from === store.lead) store.members = null;
      status.value = INFO(leftNetworkStatus(netName));
    } else if (reply.nets.length > 0) {
      store.nets = reply.nets;
    }
    wake();
  }

  async function act(action: MemberAction, idFull: string, reason?: string): Promise<void> {
    const from = slot();
    const netName = net();
    const { reply } = await postMemberAction(call, action, idFull, reason, from);
    putSlotMembers(store, from, reply);
    if (from === store.lead) store.members = reply;
    status.value = INFO(doneText(action, idFull, reply, netName));
    wake();
  }

  async function run(action: MemberAction, idFull: string, reason?: string): Promise<void> {
    try {
      if (action === "leave") await leave();
      else await act(action, idFull, reason);
    } catch (err) {
      status.value = FAILED(failureText(err));
    }
  }

  async function ask(kind: DialogKind, idFull: string): Promise<void> {
    const spec = dialogSpec(kind, context(kind, idFull));
    const yes = dialog.value ? await dialog.value.ask(spec) : false;
    if (yes) await run(ACTION_OF[kind], idFull, dialog.value?.reason?.());
  }

  const pickers: Record<string, (idFull: string) => Promise<void>> = {
    block: (id) => ask("block", id),
    confirm: (id) => ask("confirm", id),
    admit: (id) => ask("admit", id),
    unadmit: (id) => run("unadmit", id),
    exclude: (id) => ask(membersOf(store, slot())?.need === 1 ? "remove-two" : "remove", id),
    leave: () => ask("leave", meId(stateOf(store, slot()))),
    unblock: (id) => run("unblock", id),
    withdraw: (id) => run("unexclude", id),
    rename: async () => navigateTo("#home"),
  };

  const pick = async (key: string, idFull: string): Promise<void> => {
    const act = pickers[key];
    if (act) await act(idFull);
  };

  // "Join with another invite": the Home tab, the invite field. The address hash is the source of truth for the section
  // (`hashchange` re-reads it), so it moves together with the store: otherwise a pending `hashchange` of the previous
  // tab switches the page back, and a reload or "Back" lands on the old tab.
  async function joinAnother(): Promise<void> {
    location.hash = "home";
    store.activeSection = "home";
    await nextTick();
    document.getElementById("join-key-input")?.focus();
  }

  return { status, dialog, ask, run, pick, joinAnother };
}
