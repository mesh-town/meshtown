/* Actions of the Members pane of a network tab: ask in the dialog, send to `POST /api/members/<action>`, keep the answer in
 * `store.members`, wake the poller, and produce the line under the list. Does not draw anything. */

import { ref } from "vue";
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
import { FAILED, INFO, errorCode, failureText } from "./action-status.ts";
import { leaveNet, postMemberAction } from "./members-api.ts";
import { useManagedActions } from "./use-managed-actions.ts";
import { admittedStatus, blockedStatus, bannedStatus, leftNetworkStatus } from "./members-texts.ts";
import type { MemberAction } from "./members-types.ts";
import { displayNames, leaveKind, managedFacts, meId, memberById } from "./members-view.ts";

/** What `ConfirmDialog` exposes outward through `defineExpose`. */
interface DialogHandle {
  ask: (spec: DialogSpec) => Promise<boolean>;
  reason?: () => string;
  alt?: () => boolean; // the last opening was closed with the second button
}

interface MembersActionsDeps {
  api?: ApiFn;
  busTarget?: EventTarget;
}

interface MembersActions {
  status: Ref<ExitSaveStatus>;
  dialog: Ref<DialogHandle | null>;
  ask: (kind: DialogKind, idFull: string) => Promise<void>;
  run: (action: MemberAction, idFull: string, reason?: string, from?: string) => Promise<void>;
  pick: (key: string, idFull: string) => Promise<void>;
}

const ACTION_OF: Partial<Record<DialogKind, MemberAction>> = { "ban-managed": "exclude", admit: "admit", block: "block", leave: "leave" };

export function useMembersActions(store: Store, slot: () => string = () => store.lead, deps: MembersActionsDeps = {}): MembersActions {
  const status = ref<ExitSaveStatus>({ text: "", cls: "" });
  const dialog = ref<DialogHandle | null>(null);
  const call: ApiFn = deps.api || coreApi;
  const multi = (): boolean => isMulti(store);
  const net = (at: string = slot()): string => (multi() ? slotNetName(store.nets, at) : stateOf(store, at)?.network?.label || store.nets.find((n) => n.id === at)?.label || "");
  const facts = (at: string = slot()) => managedFacts(stateOf(store, at)?.network, membersOf(store, at), meId(stateOf(store, at)));
  const nameOf = (idFull: string, at: string = slot()): string => displayNames(membersOf(store, at))[idFull] || idFull.slice(0, 8);

  // `from` is the network the dialog names: the action must go to the same one after the window closes.
  function context(kind: DialogKind, idFull: string, from: string) {
    const row = memberById(membersOf(store, from), idFull);
    const label = row && row.admission ? row.admission.label : null;
    const members = membersOf(store, from);
    const exitHere = kind === "delete-net" ? exitInNet(from, true) : null;
    return { name: nameOf(idFull, from), label, net: net(from), isMyExit: exitHere ? true : chosenExitIds(store).includes(idFull), multi: multi(), exitName: exitHere || exitInNet(from), members: members?.counts.members };
  }

  // My exit, when it is a member of this network.
  function exitInNet(from: string, always = false): string | null {
    const inNet = chosenExitIds(store).find((id) => memberById(membersOf(store, from), id));
    return (always || multi()) && inNet ? nameOf(inNet, from) : null;
  }

  function wake(): void {
    const target = deps.busTarget || (typeof document === "undefined" ? undefined : document);
    if (target) notifyChanged(target, "state");
  }

  const managed = useManagedActions({ store, call, status, wake, nameOf, net });

  // The line under the list after an accepted action, by action.
  function doneText(action: MemberAction, idFull: string, from: string): string {
    const name = nameOf(idFull, from);
    const done: Record<MemberAction, () => string> = {
      unblock: () => "",
      unexclude: () => "",
      block: () => blockedStatus(name),
      admit: () => admittedStatus(name),
      exclude: () => bannedStatus(name),
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
    status.value = INFO(doneText(action, idFull, from));
    wake();
  }

  async function run(action: MemberAction, idFull: string, reason?: string, from: string = slot()): Promise<void> {
    try {
      if (action === "leave") await leave(from);
      else await act(action, idFull, reason, from);
    } catch (err) {
      if (action === "leave" && errorCode(err) === "last_admin") await ask("last-admin", idFull);
      else status.value = FAILED(failureText(err));
    }
  }

  // The answer to a managed dialog: roles and deleting go through their own calls, the rest is a member action.
  async function confirmed(kind: DialogKind, idFull: string, from: string): Promise<void> {
    const reason = dialog.value?.reason?.();
    if (kind === "make-admin") await managed.setAdmin(idFull, true, from);
    else if (kind === "give-up") await managed.setAdmin(idFull, false, from);
    else if (kind === "delete-net") await managed.deleteNet(from);
    else if (kind !== "last-admin") await run(ACTION_OF[kind]!, idFull, reason, from);
  }

  // The second button of "last-admin" goes on to deleting the network.
  async function ask(kind: DialogKind, idFull: string): Promise<void> {
    const from = slot();
    const spec = dialogSpec(kind, context(kind, idFull, from));
    const yes = dialog.value ? await dialog.value.ask(spec) : false;
    if (yes) await confirmed(kind, idFull, from);
    else if (kind === "last-admin" && dialog.value?.alt?.()) await ask("delete-net", idFull);
  }

  const pickers: Record<string, (idFull: string) => Promise<void>> = {
    block: (id) => ask("block", id),
    admit: (id) => ask("admit", id),
    exclude: (id) => ask("ban-managed", id),
    leave: () => ask(leaveKind(facts()), meId(stateOf(store, slot()))),
    "make-admin": (id) => ask("make-admin", id),
    "make-member": (id) => managed.setAdmin(id, false, slot()),
    "give-up": (id) => ask("give-up", id),
    "delete-network": (id) => ask("delete-net", id),
    unblock: (id) => run("unblock", id),
    rename: async () => navigateTo("#home"),
  };

  const pick = async (key: string, idFull: string): Promise<void> => {
    const act = pickers[key];
    if (act) await act(idFull);
  };

  return { status, dialog, ask, run, pick };
}
