/* The whole page on a fake node with a member list, for the flows of the "Members" tab (`PeersSection.members*.vtest.mjs`).
 * `openMembersPage({ members, state, tab })` mounts the page, serves `members` on `GET /api/members` (`members: null` — leave the
 * built-in 404 of an older node) and returns a page object: `when*` methods act as a user does, `expect*` methods check what is on
 * the screen. The server side is `page.node`: `answerWith(action, reply)` / `failWith(action, status, error, message)` set the
 * `POST /api/members/<action>` routes; an answer also becomes what the next `GET /api/members` returns.
 * Selectors form the contract of the page: rows `#peers-list > li`, the menu `button[data-member-menu]` and
 * `[role=menuitem][data-member-item]`, the dialog `#confirm-title` / `#confirm-body` / `#member-confirm-yes` / `#member-confirm-cancel`. */

import { expect } from "vitest";
import { demoConfig, demoState } from "./demo-node.mjs";
import { createFakeNode } from "./fake-node.mjs";
import { actionReply, membersReply, peerOneRow, peerTwoRow, selfRow } from "./members-fixtures.mjs";
import { shown } from "./dom.mjs";
import { mountPage, openTab, polls, settle } from "./page.mjs";

export const ME = { code: "aaaa1111", name: "finch-unwary" };
export const ONE = { code: "bbbb2222", name: "peer-one" };
export const TWO = { code: "cccc3333", name: "peer-two" };
export const STRAY = { code: "dddd4444", name: "old-laptop" };
export const NET = "demo-network";

const DIALOG = 'dialog[aria-labelledby="confirm-title"]';

// You, peer-one and peer-two (the three of `demoState`); `over` is merged into the reply (`need`, `me_excluded` …).
export const threeMembers = (over = {}) => membersReply({ rows: [selfRow(), peerOneRow(), peerTwoNamed()], ...over });
export const peerTwoNamed = (over = {}) => peerTwoRow({ name: TWO.name, online: true, ...over });

// Only you and peer-one: `need` is 1, a single confirmation removes.
export const twoMembers = (over = {}) => membersReply({ rows: [selfRow(), peerOneRow()], need: 1, ...over });

// `/api/state` of the demo node with the members summary of the contract.
export const stateWith = (summary = {}) => {
  const state = demoState();
  return { ...state, members: { count: state.peers.length, gossip_neighbors: 0, voting: 0, me_excluded: false, ...summary } };
};

// `exits`: the exit members chosen in the config (`route.exits`), e.g. `[PEER_FULL]` — then peer-one "is your exit".
export async function openMembersPage({ members = threeMembers(), state = stateWith(), tab = "peers", exits = [], legacy = false } = {}) {
  localStorage.clear();
  const node = createFakeNode({ state, config: demoConfig({ route: { exits } }), legacy });
  node.members = members;
  if (members !== null) node.routes["GET /api/members"] = () => node.members;
  const { wrapper, store } = await mountPage(node);
  if (tab) await openTab(wrapper, tab);
  return pageObject(node, wrapper, store);
}

function pageObject(node, wrapper, store) {
  const peerRow = (code) => wrapper.findAll("#peers-list > li").find((li) => li.text().includes(code));
  const dialog = () => wrapper.find(DIALOG);
  return {
    node, wrapper, store, peerRow, dialog,
    ...serverSide(node),
    ...userActions(wrapper, node, peerRow, dialog),
    ...screen(wrapper, node, peerRow, dialog),
  };
}

function serverSide(node) {
  return {
    answerWith(action, reply, changed = true) {
      node.routes["POST /api/members/" + action] = () => {
        node.members = reply;
        return actionReply(reply, changed);
      };
    },
    failWith(action, status, error, message) {
      node.routes["POST /api/members/" + action] = { status, body: { error, message } };
    },
    // The list request itself fails: 409 `busy` (the node is not in a network now) / the network is gone.
    membersBusy() { node.routes["GET /api/members"] = { status: 409, body: { error: "busy", message: "The node is not in a network" } }; },
    membersUnreachable() { node.routes["GET /api/members"] = () => { throw new TypeError("Failed to fetch"); }; },
    postedBodies: (action) => node.calls.filter((c) => c.method === "POST" && c.path === "/api/members/" + action).map((c) => c.body),
    members: (reply) => { node.members = reply; },
  };
}

function userActions(wrapper, node, peerRow, dialog) {
  const click = async (w) => { await w.trigger("click"); await settle(); };
  return {
    openMenu: (idFull) => click(wrapper.get('button[data-member-menu="' + idFull + '"]')),
    pick: (key) => click(wrapper.get('[role="menu"] [data-member-item="' + key + '"]')),
    confirmDialog: () => click(wrapper.get("#member-confirm-yes")),
    cancelDialog: () => click(wrapper.get("#member-confirm-cancel")),
    clickRowAction: (code, kind) => click(peerRow(code).get('[data-member-action="' + kind + '"]')),
    click: (selector) => click(wrapper.get(selector)),
    // What the browser does on Esc inside an open modal dialog.
    async pressEscapeInDialog() {
      const el = dialog().element;
      const cancel = new Event("cancel", { cancelable: true });
      el.dispatchEvent(cancel);
      if (!cancel.defaultPrevented) el.close();
      await settle();
    },
    async waitPolls(n = 2) { await polls(node, n); },
  };
}

function screen(wrapper, node, peerRow, dialog) {
  const text = (selector) => wrapper.get(selector).text();
  const isDialogOpen = () => dialog().exists() && dialog().element.hasAttribute("open");
  return {
    status: () => text("#peers-exit-status"),
    badge: (code) => (peerRow(code).find(".peer-badge").exists() ? peerRow(code).get(".peer-badge").text() : null),
    lines: (code) => peerRow(code).findAll(".peer-line").map((l) => l.text()),
    rowAction: (code, kind) => peerRow(code).find('[data-member-action="' + kind + '"]'),
    hasRowAction: (code, kind) => peerRow(code).find('[data-member-action="' + kind + '"]').exists(),
    toggleHasFocus: (idFull) => document.activeElement === wrapper.get('button[data-member-menu="' + idFull + '"]').element,
    menuLabels: () => wrapper.findAll('[role="menu"] [role="menuitem"]').map((i) => i.text()),
    isDialogOpen,
    expectDialog({ title, action }) {
      expect(isDialogOpen(), "the confirmation dialog is open").toBe(true);
      expect(text("#confirm-title")).toBe(title);
      expect(text("#member-confirm-yes")).toBe(action);
      expect(text("#member-confirm-cancel")).toBe("Cancel");
      expect(document.activeElement, "the focus is on Cancel").toBe(wrapper.get("#member-confirm-cancel").element);
    },
    dialogBody: () => text("#confirm-body"),
    listCodes: () => wrapper.findAll("#peers-list .peer-code").map((c) => c.text()),
    has: (selector) => wrapper.find(selector).exists(),
    text,
    attr: (selector, name) => wrapper.get(selector).attributes(name),
    statusLine: () => text("#status-line"),
    tabShown: (id) => shown(wrapper.get("#sec-" + id).element),
    focusedId: () => document.activeElement && document.activeElement.id,
    summaryOf: (selector) => text(selector + " > summary"),
    askedMembers: () => node.count("GET", "/api/members"),
  };
}
