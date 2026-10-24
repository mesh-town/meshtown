/* Actions of the Members tab: ask in the dialog, send to `POST /api/members/<action>`, keep the answer in
 * `store.members`, wake the poller, and produce the line under the list. Does not draw anything. */

import { nextTick, ref } from "vue";
import type { Ref } from "vue";
import { api as coreApi } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import { notifyChanged } from "../core/bridge.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { dropSlot, putSlotMembers } from "../core/overview.ts";
import { isMulti, membersOf, netName as slotNetName, stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { chosenExitIds } from "../common/exits.ts";
import { navigateTo } from "../common/navigate.ts";
import { dialogSpec } from "./dialog-specs.ts";
import type { DialogKind, DialogSpec } from "./dialog-specs.ts";
import { leaveNet, memberErrorText, postMemberAction } from "./members-api.ts";
import { admittedStatus, blockedStatus, confirmedStatus, leftNetworkStatus, banProposedStatus, bannedStatus, forgetProposedStatus, forgottenStatus, unforgottenStatus, unadmittedStatus, withdrawnStatus } from "./members-texts.ts";
import type { MemberAction, MembersReply } from "./members-types.ts";
import { banAtRisk, displayNames, meId, memberById, onlyOtherMember } from "./members-view.ts";

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
  run: (action: MemberAction, idFull: string, reason?: string, from?: string) => Promise<void>;
  pick: (key: string, idFull: string) => Promise<void>;
  joinAnother: () => Promise<void>;
}

const ACTION_OF: Record<DialogKind, MemberAction> = { ban: "exclude", "ban-two": "exclude", "ban-confirm": "exclude", admit: "admit", block: "block", leave: "leave", forget: "forget", "forget-two": "forget", "forget-confirm": "forget" };
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
  const multi = (): boolean => isMulti(store);
  const net = (at: string = slot()): string => (multi() ? slotNetName(store.nets, at) : stateOf(store, at)?.network?.label || "");
  const nameOf = (idFull: string, at: string = slot()): string => displayNames(membersOf(store, at))[idFull] || idFull.slice(0, 8);

  // `from` is the network the dialog names: the action must go to the same one after the window closes.
  function context(kind: DialogKind, idFull: string, from: string) {
    const row = memberById(membersOf(store, from), idFull);
    const by = row && row.vote ? nameOf(row.vote.proposed_by, from) : undefined;
    const label = row && row.admission ? row.admission.label : null;
    const members = membersOf(store, from);
    const other = onlyOtherMember(members, meId(stateOf(store, from)), idFull);
    const risk = banAtRisk(members, idFull);
    return { name: nameOf(idFull, from), by, seen: row ? row.last_seen_sec : null, other: other ? nameOf(other, from) : undefined, banAtRisk: risk, label, reason: row && row.vote ? row.vote.reason : null, net: net(from), isMyExit: chosenExitIds(store).includes(idFull), multi: multi(), exitName: exitInNet(from) };
  }

  // My exit, when it is a member of this network.
  function exitInNet(from: string): string | null {
    const inNet = chosenExitIds(store).find((id) => memberById(membersOf(store, from), id));
    return multi() && inNet ? nameOf(inNet, from) : null;
  }

  function wake(): void {
    const target = deps.busTarget || (typeof document === "undefined" ? undefined : document);
    if (target) notifyChanged(target, "state");
  }

  // The line under the list after an accepted action, by action.
  function doneText(action: MemberAction, idFull: string, reply: MembersReply, from: string): string {
    const name = nameOf(idFull, from);
    const row = memberById(reply, idFull);
    const done: Record<MemberAction, () => string> = {
      unblock: () => "",
      unexclude: () => withdrawnStatus(name, row?.blocked_by_me === true),
      block: () => blockedStatus(name),
      unadmit: () => unadmittedStatus(name),
      admit: () => (row?.status === "member" ? admittedStatus(name) : confirmedStatus(name)),
      exclude: () => (row?.status === "excluded" ? bannedStatus(name) : banProposedStatus(name)),
      forget: () => (row?.status === "forgotten" ? forgottenStatus(name) : forgetProposedStatus(name)),
      unforget: () => unforgottenStatus(name),
      leave: () => "",
    };
    return done[action]();
  }

  // Leaving: a left network disappears from the page; a refused leave only refreshes the network list.
  async function leave(from: string): Promise<void> {
    const netName = net(from);
    const reply = await leaveNet(call, from);
    if (reply.left) {
      const sliced = store.slots[from] !== undefined;
      dropSlot(store, from, reply.nets);
      if (!sliced && from === store.lead) store.members = null;
      status.value = INFO(leftNetworkStatus(netName, reply.announced));
    } else if (reply.nets.length > 0) {
      store.nets = reply.nets;
    }
    wake();
  }

  async function act(action: MemberAction, idFull: string, reason: string | undefined, from: string): Promise<void> {
    const { reply } = await postMemberAction(call, action, idFull, reason, from);
    putSlotMembers(store, from, reply);
    if (from === store.lead) store.members = reply;
    status.value = INFO(doneText(action, idFull, reply, from));
    wake();
  }

  async function run(action: MemberAction, idFull: string, reason?: string, from: string = slot()): Promise<void> {
    try {
      if (action === "leave") await leave(from);
      else await act(action, idFull, reason, from);
    } catch (err) {
      status.value = FAILED(failureText(err));
    }
  }

  async function ask(kind: DialogKind, idFull: string): Promise<void> {
    const from = slot();
    const spec = dialogSpec(kind, context(kind, idFull, from));
    const yes = dialog.value ? await dialog.value.ask(spec) : false;
    if (yes) await run(ACTION_OF[kind], idFull, dialog.value?.reason?.(), from);
  }

  // The dialog for removing an offline node: the second vote confirms, a two-member network removes at once.
  function forgetKind(idFull: string): DialogKind {
    const members = membersOf(store, slot());
    if (memberById(members, idFull)?.forget) return "forget-confirm";
    return members?.need === 1 ? "forget-two" : "forget";
  }

  const pickers: Record<string, (idFull: string) => Promise<void>> = {
    block: (id) => ask("block", id),
    confirm: (id) => ask("ban-confirm", id),
    admit: (id) => ask("admit", id),
    unadmit: (id) => run("unadmit", id),
    forget: (id) => ask(forgetKind(id), id),
    unforget: (id) => run("unforget", id),
    exclude: (id) => ask(membersOf(store, slot())?.need === 1 ? "ban-two" : "ban", id),
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
