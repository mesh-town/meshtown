/* `members/UnlistedList.vue` inside the app: the block item names the phone; on a desktop it names the computer. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import UnlistedList from "./UnlistedList.vue";
import { MENU_UNBLOCK } from "./members-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const STRAY = { id: "dddd4444", id_full: "D".repeat(64), name: "old-laptop", blocked_by_me: false };
const BLOCKED = { id: "eeee5555", id_full: "E".repeat(64), name: null, blocked_by_me: true };

let wrapper;
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

async function menuOf(row, userAgent) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  wrapper = mount(UnlistedList, { props: { rows: [row] }, attachTo: document.body });
  await wrapper.get('button[data-member-menu="' + row.id_full + '"]').trigger("click");
  return wrapper.findAll('[role="menuitem"]').map((i) => [i.attributes("data-member-item"), i.text()]);
}

describe("UnlistedList menu in the app", () => {
  it("a node that is not blocked: Block on this phone", async () => {
    expect(await menuOf(STRAY, APP_UA)).toEqual([["block", "Block on this phone…"]]);
  });

  it("a blocked node: the unblock item", async () => {
    expect(await menuOf(BLOCKED, APP_UA)).toEqual([["unblock", MENU_UNBLOCK]]);
  });

  it("on a desktop the block item names the computer", async () => {
    expect(await menuOf(STRAY, DESKTOP_UA)).toEqual([["block", "Block on this computer…"]]);
  });
});
