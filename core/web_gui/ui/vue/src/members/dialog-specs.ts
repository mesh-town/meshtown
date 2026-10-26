/* Texts of the confirmation dialogs. */

import { forDevice } from "../common/device.ts";
import { isYourExitText, labelLine, leaveExitLine, leaveMultiBody, leaveMultiTitle } from "./members-texts.ts";
import { quotedNet } from "../common/words.ts";
import { MANAGED_BUILDERS, MANAGED_CALM, MANAGED_CALM_DANGER, type ManagedKind } from "./dialog-managed.ts";

export type DialogKind = "admit" | "block" | "leave" | ManagedKind;

// `name`: the target as shown in the list; `net`: the network name without quotes.
export interface DialogContext {
  name: string;
  net?: string | null;
  isMyExit?: boolean;
  label?: string | null; // the "for whom" label of the invite ("admit")
  multi?: boolean; // the page shows several networks ("leave")
  exitName?: string | null; // my exit, when it is a member of this network ("leave", several networks)
  members?: number; // how many members the network has ("delete-net")
}

export interface DialogSpec {
  title: string;
  body: string[];
  hint?: string;
  field?: { label: string; max: number; match?: string }; // `match`: the exact text that enables the main button
  action: string;
  alt?: { label: string; danger: boolean }; // a second button next to the main one ("last-admin")
  danger: boolean;
}

type Builder = (c: DialogContext) => Omit<DialogSpec, "danger">;

const BUILDERS: Record<DialogKind, Builder> = {
  ...MANAGED_BUILDERS,
  admit: (c) => ({
    title: "Confirm " + c.name + " joining?",
    body: [
      c.name + " asked to join the network" + quotedNet(c.net) + ". After your confirmation it joins right away.",
      ...(c.label ? [labelLine(c.label)] : []),
    ],
    hint: "Confirm only if you recognise this computer. If unsure, ask the member who invited it.",
    action: "Confirm " + c.name,
  }),
  block: (c) => ({
    title: "Block " + c.name + forDevice(" on this computer?"),
    body: [c.name + forDevice(" won't be able to connect to this computer or open sites through it. Its open connections to you are cut.")],
    hint:
      forDevice("Only this computer is affected: ") + c.name +
      ' stays in the network and keeps working with other members. To remove it for everyone, choose "Remove from network" instead.',
    action: "Block " + c.name,
  }),
  leave: (c) => (c.multi ? leaveMulti(c) : {
    title: "Leave the network" + quotedNet(c.net) + "?",
    body: [
      forDevice("This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member."),
    ],
    action: "Leave network",
  }),
};

function leaveMulti(c: DialogContext): Omit<DialogSpec, "danger"> {
  const net = c.net || "";
  return { title: leaveMultiTitle(net), body: [leaveMultiBody(net), ...(c.exitName ? [leaveExitLine(c.exitName)] : [])], action: "Leave network" };
}

export function dialogSpec(kind: DialogKind, ctx: DialogContext): DialogSpec {
  const built = BUILDERS[kind](ctx);
  const extra = ctx.isMyExit && kind !== "leave" && !MANAGED_CALM.includes(kind) ? [isYourExitText(ctx.name)] : [];
  return { ...built, body: [...built.body, ...extra], danger: kind !== "admit" && !MANAGED_CALM_DANGER.includes(kind) };
}
