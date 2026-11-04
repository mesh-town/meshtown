import { flushPromises } from "@vue/test-utils";

export const SUGGESTED_NAME = "finch-unwary";
export const INVITE_NAME = '[name="invite-name"]';

export async function openInviteDialog(wrapper, button = "#netkey-invite-btn") {
  await wrapper.get(button).trigger("click");
  await flushPromises();
  return wrapper.get("dialog[open]");
}

export async function submitInviteDialog(dialog) {
  await dialog.get("form").trigger("submit");
  await flushPromises();
}
