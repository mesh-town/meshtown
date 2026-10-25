/* Actions of a managed network on the Members tab: roles and deleting the network. The dialog and the removal
 * (`members/exclude`) are in `use-members-actions.ts`. */

import type { Ref } from "vue";
import type { ApiFn } from "../core/api.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { putSlotMembers } from "../core/overview.ts";
import type { Store } from "../core/store.ts";
import { FAILED, INFO, failureText } from "./action-status.ts";
import { useDeletedBanner } from "./deleted-banner.ts";
import { deleteNetwork, postAdmin } from "./members-api.ts";
import { madeAdminStatus } from "./members-texts.ts";

interface ManagedEnv {
  store: Store;
  call: ApiFn;
  status: Ref<ExitSaveStatus>;
  wake: () => void;
  nameOf: (idFull: string, at: string) => string;
  net: (at: string) => string;
}

interface ManagedActions {
  setAdmin: (idFull: string, admin: boolean, from: string) => Promise<void>;
  deleteNet: (from: string) => Promise<void>;
}

export function useManagedActions(env: ManagedEnv): ManagedActions {
  async function setAdmin(idFull: string, admin: boolean, from: string): Promise<void> {
    try {
      const { reply } = await postAdmin(env.call, idFull, admin, from);
      putSlotMembers(env.store, from, reply);
      if (from === env.store.lead) env.store.members = reply;
      env.status.value = INFO(admin ? madeAdminStatus(env.nameOf(idFull, from), env.net(from)) : "");
      env.wake();
    } catch (err) {
      env.status.value = FAILED(failureText(err));
    }
  }

  // The network is gone for everyone; the next snapshot shows it (the poller is woken).
  async function deleteNet(from: string): Promise<void> {
    try {
      const name = env.net(from);
      await deleteNetwork(env.call, from);
      useDeletedBanner().show(name);
      env.status.value = INFO("");
      env.wake();
    } catch (err) {
      env.status.value = FAILED(failureText(err));
    }
  }

  return { setAdmin, deleteNet };
}
