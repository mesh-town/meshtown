/* This computer's own exit turning members away, as a warning strip under the header (on every tab). */

import { membersOf, stateOf, visibleSlots } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { isMeExcluded } from "../members/members-view.ts";
import { peersHash } from "../shell/nav.ts";

export const OWN_OVERLOAD_STRIP = "⚠️ This computer is turning members away.";
export const OWN_OVERLOAD_STRIP_LINK = "See who and the connection limit";

// The tab of the first visible network whose exit of this computer is overloaded; `null` when none is.
export function ownOverloadTarget(store: Store): string | null {
  const slot = visibleSlots(store.nets, store.slots).find((s) => {
    const state = stateOf(store, s);
    return !!state?.exit?.load?.overloaded && !isMeExcluded(state, membersOf(store, s));
  });
  return slot === undefined ? null : peersHash(slot);
}
