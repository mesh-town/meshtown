/* The "Exit via …" button of the "Members" and "Site checks" sections: the member becomes the
 * single exit (`route.exits`). Returns the status for the line under the list. */

import type { SaveDeps } from "./save-config.ts";
import type { Store } from "./store.ts";
import { saveConfigFields, fieldErrorText } from "./save-config.ts";
import { setRememberedExits } from "../common/exits.ts";
import { CORE_SAVED } from "./core-texts.ts";

// `deps` — as for `saveConfigFields` plus `remember(exits)` (by default the page's localStorage).
export interface ExitSaveStatus {
  text: string;
  cls: string;
}

export async function saveSingleExit(
  store: Store,
  idFull: string,
  deps: SaveDeps & { remember?: (exits: string[]) => void } = {},
): Promise<ExitSaveStatus> {
  const changes = { "route.exits": [idFull] };
  const res = await saveConfigFields(store, changes, deps);
  if (res.ok) (deps.remember || setRememberedExits)([idFull]);
  return res.ok
    ? { text: CORE_SAVED, cls: "saved" }
    : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
}
