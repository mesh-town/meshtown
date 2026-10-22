/* Labels of buttons and badges for the networks block and the summary (explanatory texts are in `nets-texts.ts`). */

import { NET_NAME } from "../common/words.ts";

export const BADGE_AUTO_CLOSED = "closed automatically";
export const BADGE_NOT_CHECKED = "not selected";
export const BADGE_HERE = "you are here now";
export const BADGE_GONE = "not here now";
export const BTN_SAVE = "Save";
export const BTN_CANCEL = "Cancel";
export const BTN_RESTART = "Restart " + NET_NAME;
export const BTN_EDIT = "Edit";
export const BTN_OPEN_HOME = "Open for the home network";
// A plate: the program is the subject of the phrase, so `NET_NAME`, not "Node …" (guarded by `words-node.test.mjs`, like `RESTART_STARTED`).
export const PLATE_BUSY = NET_NAME + " is restarting…";
export const PLATE_DONE = NET_NAME + " has restarted";
export const RESTART_FAILED = "Couldn't restart: ";
