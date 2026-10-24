/* Texts of the confirmation dialogs. */

import {
  FORGET_PROPOSE, FORGET_RECENT_HINT, REASON_LABEL, banAtRiskText, forgetAction, forgetConfirmLine, forgetConfirmTitle, forgetEffectLine,
  forgetLastSeenLine, forgetNeedTwoLine, forgetOnlyTwoLine, forgetTitle, isYourExitText, labelLine, leaveExitLine, leaveMultiBody, leaveMultiTitle,
  reasonLine,
} from "./members-texts.ts";
import { quotedNet } from "../common/words.ts";
import { REASON_MAX } from "./member-reason.ts";
import { MANAGED_BUILDERS, MANAGED_CALM, MANAGED_CALM_DANGER, type ManagedKind } from "./dialog-managed.ts";

export type DialogKind = "ban" | "ban-two" | "ban-confirm" | "admit" | "block" | "leave" | "forget" | "forget-two" | "forget-confirm" | ManagedKind;

// `name`: the target as shown in the list; `by`: the proposer (only "confirm"); `net`: the network name without quotes.
export interface DialogContext {
  name: string;
  by?: string;
  net?: string | null;
  isMyExit?: boolean;
  reason?: string | null; // the proposer's reason ("confirm")
  label?: string | null; // the "for whom" label of the invite ("admit")
  multi?: boolean; // the page shows several networks ("leave")
  exitName?: string | null; // my exit, when it is a member of this network ("leave", several networks)
  seen?: number | null; // seconds since the target was last online; null — never ("forget")
  other?: string; // the only other member ("forget-two")
  members?: number; // how many members the network has ("delete-net")
  banAtRisk?: boolean; // a ban vote is pending and this removal lowers its threshold ("forget-two", "forget-confirm")
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
  ban: (c) => ({
    title: "Ban " + c.name + " from the network?",
    body: [
      c.name + " will lose access to the network" + quotedNet(c.net) + ". This takes two members: you propose, and one more member confirms.",
      "On this computer, " + c.name + " is blocked right away.",
      "For everyone else, it's banned after a second member confirms. Its open connections are then cut.",
    ],
    hint:
      "Use this if the computer is lost, stolen, or no longer used. To keep " + c.name +
      ' away from this computer only, choose "Block on this computer" instead.',
    field: { label: REASON_LABEL, max: REASON_MAX },
    action: "Propose ban",
  }),
  "ban-two": (c) => ({
    title: "Ban " + c.name + " from the network?",
    body: [
      "You and " + c.name + " are the only members, so " + c.name +
        " is banned right away and its open connections are cut. You can't undo this here.",
    ],
    action: "Ban " + c.name,
  }),
  "ban-confirm": (c) => ({
    title: "Confirm banning " + c.name + "?",
    body: [
      c.by + " proposed banning " + c.name + " from the network" + quotedNet(c.net) + ". Yours is the second confirmation: " +
        c.name + " will be banned for everyone right away and its open connections will be cut.",
      ...(c.reason && c.by ? [reasonLine(c.by, c.reason)] : []),
    ],
    hint: "Confirm only if you know why. If unsure, ask " + c.by + " first.",
    action: "Ban " + c.name,
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
      ' stays in the network and keeps working with other members. To ban it for everyone, choose "Ban from network" instead.',
    action: "Block " + c.name,
  }),
  forget: (c) => ({
    title: forgetTitle(c.name),
    body: [forgetLastSeenLine(c.name, c.seen ?? null), forgetEffectLine(), forgetNeedTwoLine(c.name)],
    hint: recentlySeen(c) ? FORGET_RECENT_HINT : undefined,
    action: FORGET_PROPOSE,
  }),
  "forget-two": (c) => ({
    title: forgetTitle(c.name),
    body: [forgetLastSeenLine(c.name, c.seen ?? null), forgetEffectLine(), forgetOnlyTwoLine(c.name, c.other || ""), ...atRisk(c)],
    hint: recentlySeen(c) ? FORGET_RECENT_HINT : undefined,
    action: forgetAction(c.name),
  }),
  "forget-confirm": (c) => ({
    title: forgetConfirmTitle(c.name),
    body: [forgetConfirmLine(c.name), ...atRisk(c)],
    action: forgetAction(c.name),
  }),
  leave: (c) => (c.multi ? leaveMulti(c) : {
    title: "Leave the network" + quotedNet(c.net) + "?",
    body: [
      "This computer disconnects from all members, and sites stop opening through Mesh Town. To come back, you'll need a new invite from a member.",
    ],
    action: "Leave network",
  }),
};

const WEEK_SECS = 604800;
const recentlySeen = (c: DialogContext): boolean => typeof c.seen === "number" && c.seen < WEEK_SECS;
const atRisk = (c: DialogContext): string[] => (c.banAtRisk ? [banAtRiskText(c.name)] : []);

function leaveMulti(c: DialogContext): Omit<DialogSpec, "danger"> {
  const net = c.net || "";
  return { title: leaveMultiTitle(net), body: [leaveMultiBody(net), ...(c.exitName ? [leaveExitLine(c.exitName)] : [])], action: "Leave network" };
}

export function dialogSpec(kind: DialogKind, ctx: DialogContext): DialogSpec {
  const built = BUILDERS[kind](ctx);
  const extra = ctx.isMyExit && kind !== "leave" && !MANAGED_CALM.includes(kind) ? [isYourExitText(ctx.name)] : [];
  return { ...built, body: [...built.body, ...extra], danger: kind !== "admit" && !MANAGED_CALM_DANGER.includes(kind) };
}
