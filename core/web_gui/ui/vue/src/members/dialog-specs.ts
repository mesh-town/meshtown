/* Texts of the five confirmation dialogs (UI spec, section 2.2). */

import { REASON_LABEL, isYourExitText, labelLine, reasonLine } from "./members-texts.ts";
import { quotedNet } from "../common/words.ts";
import { REASON_MAX } from "./member-reason.ts";

export type DialogKind = "remove" | "remove-two" | "confirm" | "admit" | "block" | "leave";

// `name`: the target as shown in the list; `by`: the proposer (only "confirm"); `net`: the network name without quotes.
export interface DialogContext {
  name: string;
  by?: string;
  net?: string | null;
  isMyExit?: boolean;
  reason?: string | null; // the proposer's reason ("confirm")
  label?: string | null; // the "for whom" label of the invite ("admit")
}

export interface DialogSpec {
  title: string;
  body: string[];
  hint?: string;
  field?: { label: string; max: number };
  action: string;
  danger: boolean;
}

type Builder = (c: DialogContext) => Omit<DialogSpec, "danger">;

const BUILDERS: Record<DialogKind, Builder> = {
  remove: (c) => ({
    title: "Remove " + c.name + " from the network?",
    body: [
      c.name + " will lose access to the network" + quotedNet(c.net) + ". This takes two members: you propose, and one more member confirms.",
      "On this computer, " + c.name + " is blocked right away.",
      "For everyone else, it's removed after a second member confirms. Its open connections are then cut.",
    ],
    hint:
      "Use this if the computer is lost, stolen, or no longer used. To keep " + c.name +
      ' away from this computer only, choose "Block on this computer" instead.',
    field: { label: REASON_LABEL, max: REASON_MAX },
    action: "Propose removal",
  }),
  "remove-two": (c) => ({
    title: "Remove " + c.name + " from the network?",
    body: [
      "You and " + c.name + " are the only members, so " + c.name +
        " is removed right away and its open connections are cut. You can't undo this here.",
    ],
    action: "Remove " + c.name,
  }),
  confirm: (c) => ({
    title: "Confirm removing " + c.name + "?",
    body: [
      c.by + " proposed removing " + c.name + " from the network" + quotedNet(c.net) + ". Yours is the second confirmation: " +
        c.name + " will be removed for everyone right away and its open connections will be cut.",
      ...(c.reason && c.by ? [reasonLine(c.by, c.reason)] : []),
    ],
    hint: "Confirm only if you know why. If unsure, ask " + c.by + " first.",
    action: "Remove " + c.name,
  }),
  admit: (c) => ({
    title: "Confirm " + c.name + " joining?",
    body: [
      c.name + " asked to join the network" + quotedNet(c.net) + ". After your confirmation it joins as soon as enough members have confirmed it.",
      ...(c.label ? [labelLine(c.label)] : []),
    ],
    hint: "Confirm only if you recognise this computer. If unsure, ask the member who invited it.",
    action: "Confirm " + c.name,
  }),
  block: (c) => ({
    title: "Block " + c.name + " on this computer?",
    body: [c.name + " won't be able to connect to this computer or open sites through it. Its open connections to you are cut."],
    hint:
      "Only this computer is affected: " + c.name +
      ' stays in the network and keeps working with other members. To remove it for everyone, choose "Remove from network" instead.',
    action: "Block " + c.name,
  }),
  leave: (c) => ({
    title: "Leave the network" + quotedNet(c.net) + "?",
    body: [
      "This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
    ],
    action: "Leave network",
  }),
};

export function dialogSpec(kind: DialogKind, ctx: DialogContext): DialogSpec {
  const built = BUILDERS[kind](ctx);
  const extra = ctx.isMyExit && kind !== "leave" ? [isYourExitText(ctx.name)] : [];
  return { ...built, body: [...built.body, ...extra], danger: kind !== "admit" };
}
