/* "You deleted <net>": the line the admin sees after deleting a network, until OK. */

import { ref } from "vue";
import type { Ref } from "vue";

const deleted = ref<string | null>(null);

export const DELETED_OK = "OK";
export const deletedBannerText = (net: string): string => `You deleted "${net}". All members have left it.`;

interface DeletedBannerApi {
  deleted: Ref<string | null>;
  show: (net: string) => void;
  clear: () => void;
}

export function useDeletedBanner(): DeletedBannerApi {
  return { deleted, show: (net) => (deleted.value = net), clear: () => (deleted.value = null) };
}
