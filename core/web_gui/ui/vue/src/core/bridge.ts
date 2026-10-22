/* The sections → shell bus: a DOM event `meshtown:changed` on `document`. */

import { coreBridgeUnknownWhatText } from "./core-texts.ts";

export const EV_CHANGED = "meshtown:changed";

// Nobody sends "render": the word is kept for bus compatibility.
const CHANGED_WHAT: readonly string[] = ["config", "state", "token", "render", "stopped", "stopping"];

export type ChangedWhat = "config" | "state" | "token" | "render" | "stopped" | "stopping";

export function notifyChanged(target: EventTarget, what: ChangedWhat): void {
  if (!CHANGED_WHAT.includes(what)) throw new TypeError(coreBridgeUnknownWhatText(what));
  target.dispatchEvent(new CustomEvent(EV_CHANGED, { detail: { what } }));
}
