/* Members texts about this device inside the app: "phone" instead of "computer"; on a desktop the words stay. A name that
 * comes from the node is never rewritten, and "the computer" of a lost device in the ban hint stays. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { dialogSpec } from "./dialog-specs.ts";
import { rowDecor, menuItems } from "./members-rows.ts";
import { deleteNetBody } from "./members-managed-texts.ts";
import * as t from "./members-texts.ts";
import { memberRow, peerOneRow } from "../testing/members-fixtures.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const noComputer = (text) => assert.ok(!/computer/i.test(text), text);

const ME = "a".repeat(64);
const blockedRow = () => peerOneRow({ blocked_by_me: true });
const ctx = { me: ME };

describe("status texts", () => {
  const cases = [
    ["banProposedStatus", () => t.banProposedStatus("old-laptop"), "old-laptop is already blocked on this"],
    ["withdrawnStatus", () => t.withdrawnStatus("old-laptop", false), "old-laptop is no longer blocked on this"],
    ["blockedStatus", () => t.blockedStatus("old-laptop"), "old-laptop is blocked on this"],
    ["meWaitingText", () => t.meWaitingText(1, 2), "This %s is waiting for network members"],
    ["leftNetworkStatus", () => t.leftNetworkStatus("net", false), "may still see this %s in their list"],
    ["forgetNeedTwoLine", () => t.forgetNeedTwoLine("old-laptop"), "On this %s, old-laptop is hidden"],
    ["forgetProposedStatus", () => t.forgetProposedStatus("old-laptop"), "old-laptop is hidden on this %s."],
    ["deleteNetBody", () => deleteNetBody(3, "net"), "this %s too."],
  ];
  it.each(cases)("%s: the app says phone, the desktop says computer", (_n, make, part) => {
    given(APP_UA);
    assert.ok(make().includes(part.replace("%s", "phone")), make());
    noComputer(make());
    given(DESKTOP_UA);
    assert.ok(make().includes(part.replace("%s", "computer")), make());
  });

  it("a member named after a computer keeps its name in the app", () => {
    given(APP_UA);
    assert.equal(t.blockedStatus("my computer"), "my computer is blocked on this phone.");
  });
});

describe("the forgotten card text", () => {
  it("has no device word and no key", () => {
    given(DESKTOP_UA);
    noComputer(t.FORGOTTEN_CARD_TEXT);
    assert.ok(!/key/.test(t.FORGOTTEN_CARD_TEXT));
  });
});

describe("rows and menus", () => {
  it("the blocked badge and the line of a vote name the phone in the app, the computer on a desktop", () => {
    given(APP_UA);
    const row = blockedRow();
    assert.equal(rowDecor({ id_full: row.id_full }, row, ME).badge.text, "Blocked on this phone");
    given(DESKTOP_UA);
    assert.equal(rowDecor({ id_full: row.id_full }, row, ME).badge.text, "Blocked on this computer");
  });

  it("the Block menu item", () => {
    const row = peerOneRow();
    given(APP_UA);
    assert.equal(menuItems(row, ctx).find((i) => i.key === "block").label, "Block on this phone…");
    given(DESKTOP_UA);
    assert.equal(menuItems(row, ctx).find((i) => i.key === "block").label, "Block on this computer…");
  });
});

describe("dialogs", () => {
  const dctx = { name: "old-laptop", net: "net", id: "bbbb2222" };
  const text = (kind) => { const s = dialogSpec(kind, dctx); return [s.title, ...s.body, s.hint || ""].join(" | "); };

  it("block: every own mention is the phone", () => {
    given(APP_UA);
    noComputer(text("block"));
    assert.ok(text("block").includes("on this phone?"), text("block"));
    given(DESKTOP_UA);
    assert.ok(text("block").includes("on this computer?"));
  });

  it("ban: the own part is the phone, the lost device stays a computer", () => {
    given(APP_UA);
    const s = text("ban");
    assert.ok(s.includes("On this phone, old-laptop is blocked right away."), s);
    assert.ok(s.includes("if the computer is lost"), s);
    assert.ok(s.includes('away from this phone only, choose "Block on this phone"'), s);
    given(DESKTOP_UA);
    assert.ok(text("ban").includes("On this computer, old-laptop"));
  });

  it("admit: the recognised joiner stays a computer", () => {
    given(APP_UA);
    assert.ok(text("admit").includes("recognise this computer"));
  });
});
