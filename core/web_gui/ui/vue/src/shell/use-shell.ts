/* The shell wrapper: connection, polling, navigation and computed views. Logic lives in `conn.ts`,
 * `poller.ts` (the `meshtown:changed` bus — `poller.onBus`), `nav.ts`, `status-line.ts`, `token-url.ts`; here only `reactive`/`computed`/subscriptions. */

import { computed, onMounted, onUnmounted, reactive, watch } from "vue";
import type { ComputedRef } from "vue";
import type { Store } from "../core/store.ts";
import { setToken } from "../core/token.ts";
import { absence, createConn, stripView } from "./conn.ts";
import type { Conn, Strip, StripViewKind } from "./conn.ts";
import { openableSection, peersHash, peersTail, pickPeersTab } from "./nav.ts";
import { addressInvite } from "../home/use-invite-join.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { useHeldRefusal } from "../peers/use-held-refusal.ts";
import { routeLine } from "../home/route-line.ts";
import { createPoller } from "./poller.ts";
import { defaultShellDeps } from "./shell-deps.ts";
import type { ShellDeps } from "./shell-deps.ts";
import { identityStrip, statusLine } from "./status-line.ts";
import type { IdentityStripView, StatusLine } from "./status-line.ts";
import { tokenFromPasted } from "./token-url.ts";
import { peersSlots, refusedFlag } from "../core/slots.ts";
import { refusedNow } from "../peers/admission-refused.ts";
import { chosenExitIds } from "../common/exits.ts";
import { exitWarning, withExitWarning } from "../members/exit-members.ts";
import { isMeExcluded } from "../members/members-ids.ts";

// Listeners for `hashchange` (`deps.win`) and `visibilitychange` / `meshtown:changed` (`deps.doc`).
function listen(target: EventTarget | undefined, name: string, handler: (ev: Event) => void): () => void {
  if (!target) return () => {};
  target.addEventListener(name, handler);
  return () => target.removeEventListener(name, handler);
}

export interface Shell {
  conn: Conn;
  status: ComputedRef<StatusLine>;
  strip: ComputedRef<Strip<StripViewKind> | null>;
  identity: ComputedRef<IdentityStripView | null>;
  ready: ComputedRef<boolean>;
  applyToken: (text: unknown) => void;
  reloadConfig: () => Promise<void>;
  resume: () => void;
}

export function useShell(store: Store, deps: ShellDeps = defaultShellDeps): Shell {
  const conn = reactive(createConn());
  const poller = createPoller(store, conn, deps);
  watch(() => absence(conn), (a) => { store.absence = a; }, { immediate: true, flush: "sync" });
  store.activeSection = openableSection(deps.hash, inAppPage());
  let wanted = peersTail(deps.hash);

  // Before the first snapshot the wanted tab is kept as is; later it must be a visible slot (or "add").
  const syncPeersTab = (): void => {
    if (store.source === null) {
      store.peersTab = wanted;
      return;
    }
    store.peersTab = pickPeersTab(wanted, peersSlots(store.nets, store.slots));
  };
  syncPeersTab();
  // An invite in the address and a node that is already in a network: the form for it is "Add another network".
  let addressChecked = false;
  const checkAddress = (): void => {
    if (addressChecked || store.state === null) return;
    addressChecked = true;
    if (addressInvite() && store.state.network) location.hash = peersHash("add");
  };
  watch(() => store.state, checkAddress, { flush: "sync" });
  checkAddress();
  watch(() => peersSlots(store.nets, store.slots).join(",") + "|" + String(store.source === null), syncPeersTab, { flush: "pre" });

  const refused = useHeldRefusal(() => refusedNow(refusedFlag(store, store.lead), store.members));
  const status = computed(() => withExitWarning(statusLine(store.state, conn, store.members, routeLine(store), refused.value), exitWarning(chosenExitIds(store), store.members, isMeExcluded(store.state, store.members))));
  const strip = computed(() => stripView(conn));
  const identity = computed(() => identityStrip(store.state, store.configFile));
  const ready = computed(() => store.state !== null && store.configRev !== null);

  const onHash = () => {
    store.activeSection = openableSection(deps.hash, inAppPage());
    wanted = peersTail(deps.hash);
    syncPeersTab();
    deps.scrollToTop();
  };
  const onVisible = () => {
    if (deps.isHidden() || conn.awaitingToken || poller.stoppedRecently()) return;
    poller.schedule(0);
  };
  const onBus = (ev: Event): void => poller.onBus((ev as CustomEvent<{ what?: string } | null>).detail?.what);

  let offs: Array<() => void> = [];
  onMounted(() => {
    offs = [listen(deps.win, "hashchange", onHash), listen(deps.doc, "visibilitychange", onVisible), listen(deps.doc, "meshtown:changed", onBus)];
    poller.start();
  });
  onUnmounted(() => {
    offs.forEach((off) => off());
    offs = [];
    poller.stop();
  });

  // Polling resumes: the token is accepted or a session is obtained (password sign-in).
  function resume(): void {
    conn.awaitingToken = false;
    conn.strip = null;
    poller.schedule(0);
  }

  function applyToken(text: unknown): void {
    const token = tokenFromPasted(text, deps.href);
    if (!token) return;
    setToken(token, deps.storage);
    resume();
  }

  return { conn, status, strip, identity, ready, applyToken, reloadConfig: () => poller.refreshConfig(), resume };
}
