/* Members of a managed network: the `⋯` menu (`menuItems`), the root lookup (`isRoot`), the leave dialog choice (`leaveKind`)
 * and the voting switch (`votingHidden`). A peer network keeps its menus: `members-view-rows.test.mjs`.
 *
 * Contract fixed by these tests (all four functions are reachable through `members-view.ts`):
 *  - `menuItems(row, ctx)`, `ctx = { me, managed?, meAdmin?, isRoot?, onlyAdmin? }`: `managed` — the network is managed,
 *      `meAdmin` — this computer is an admin, `isRoot(idFull) => boolean` — the row is the network creator, `onlyAdmin` — this
 *      computer is the only admin. Without `managed` the menu is the peer menu, whatever the other fields say.
 *      New item keys: "make-admin", "make-member", "give-up", "delete-network". No separator items; a disabled item has `disabled: true`.
 *      Managed, someone else's row, I am an admin: ["make-admin" | "make-member" (the row is an admin), "exclude", "block"];
 *        no vote, withdraw, waiting, unadmit or forget items. "make-member" is disabled on the creator; no `isRoot` — never disabled.
 *      Managed, someone else's row, I am not an admin: "block" and a disabled "exclude" ("Remove from network (admins only)").
 *      Managed, my row: an admin gets "rename", "give-up", "leave", "delete-network" (red); the only admin's "give-up" is disabled
 *        and says so in its label; a plain member gets "rename", "leave".
 *  - `isRoot(info, idFull)`: `info.root === idFull`; no `info`, no `root` field — false.
 *  - `leaveKind(ctx)`, `ctx = { meAdmin?, canLeave?, onlyAdmin? }`: "last-admin" for an admin who cannot leave (`canLeave === false`)
 *      or is the only admin, otherwise "leave".
 *  - `votingHidden(reply)`: true for a managed network (`reply.kind === "Managed"`), false for a peer network, a reply without
 *      `kind` and no reply. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { isRoot, leaveKind, menuItems, votingHidden } from "./members-view.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { admissionInfo, extraRow, forgetInfo, foreignVote, peerOneRow, peerTwoRow, pendingRow, selfRow, excludedInfo, voteInfo } from "../testing/members-fixtures.mjs";

const ME = SELF_FULL;

describe("menuItems of a peer network", () => {
  describe("the context knows nothing about kinds", () => {
    it("someone else's member: block and ban", () => {
      assert.deepEqual(keys(menuItems(peerOneRow(), { me: ME })), ["block", "exclude"]);
    });

    it("my row: rename and leave", () => {
      assert.deepEqual(menuItems(selfRow(), { me: ME }).map((i) => i.label), ["Rename", "Leave network…"]);
    });
  });

  describe("managed is false although the context carries admin data", () => {
    it("the menu stays the peer menu", () => {
      const ctx = { me: ME, managed: false, meAdmin: true, isRoot: () => true };
      assert.deepEqual(keys(menuItems(peerOneRow({ admin: true }), ctx)), ["block", "exclude"]);
      assert.deepEqual(keys(menuItems(selfRow({ admin: true }), ctx)), ["rename", "leave"]);
    });
  });
});

describe("menuItems of a managed network", () => {
  describe("I am an admin, someone else's row is a plain member", () => {
    it("offers make admin, remove, block, in this order", () => {
      const items = menuItems(peerOneRow({ admin: false }), adminCtx());
      assert.deepEqual(keys(items), ["make-admin", "exclude", "block"]);
      assert.deepEqual(items.map((i) => i.label), ["Make admin", "Remove from network…", "Block on this computer…"]);
    });

    it("every item is enabled", () => {
      const items = menuItems(peerOneRow(), adminCtx());
      assert.ok(items.every((i) => !i.disabled), "an item is disabled: " + JSON.stringify(items));
    });

    it("an offline member gets no removal-by-vote item", () => {
      const row = peerTwoRow({ online: false, forget: forgetInfo({ mine: false }), last_seen_sec: 9999999 });
      assert.deepEqual(keys(menuItems(row, adminCtx())), ["make-admin", "exclude", "block"]);
    });

    it("a stale ban vote on the row gives no withdraw or waiting item", () => {
      for (const vote of [voteInfo(), foreignVote(), foreignVote({ mine: true })]) {
        assert.deepEqual(keys(menuItems(peerOneRow({ vote }), adminCtx())), ["make-admin", "exclude", "block"]);
      }
    });

    it("a stale admission on the row gives no withdraw-confirmation item", () => {
      const row = peerOneRow({ admission: admissionInfo({ mine: true }) });
      assert.deepEqual(keys(menuItems(row, adminCtx())), ["make-admin", "exclude", "block"]);
    });

    it("an excluded row has no menu", () => {
      assert.deepEqual(menuItems(extraRow({ status: "excluded", excluded: excludedInfo() }), adminCtx()), []);
    });
  });

  describe("I am an admin, someone else's row is an admin", () => {
    it("offers make regular member instead of make admin", () => {
      const items = menuItems(peerOneRow({ admin: true }), adminCtx());
      assert.deepEqual(keys(items), ["make-member", "exclude", "block"]);
      assert.equal(items[0].label, "Make regular member");
      assert.ok(!items[0].disabled, "make regular member is disabled for an ordinary admin");
    });

    it("the network creator's make-regular-member item is disabled", () => {
      const items = menuItems(peerOneRow({ admin: true }), adminCtx({ isRoot: (id) => id === PEER_FULL }));
      const item = items.find((i) => i.key === "make-member");
      assert.equal(item.disabled, true, "the creator can be made a regular member");
      assert.ok(item.label.startsWith("Make regular member"), item.label);
    });

    it("asks isRoot about the id of the row", () => {
      const isRootFn = vi.fn(() => false);
      menuItems(peerOneRow({ admin: true }), adminCtx({ isRoot: isRootFn }));
      assert.ok(isRootFn.mock.calls.some(([id]) => id === PEER_FULL), "isRoot was not asked about the row: " + JSON.stringify(isRootFn.mock.calls));
    });

    it("another admin who is not the creator stays enabled", () => {
      const items = menuItems(peerOneRow({ admin: true }), adminCtx({ isRoot: (id) => id === PEER2_FULL }));
      assert.ok(!items.find((i) => i.key === "make-member").disabled);
    });

    it("without an isRoot the item is never disabled", () => {
      const items = menuItems(peerOneRow({ admin: true }), { me: ME, managed: true, meAdmin: true });
      assert.ok(!items.find((i) => i.key === "make-member").disabled);
    });
  });

  describe("I am not an admin, someone else's row", () => {
    it("offers block and a disabled remove that says it is for admins", () => {
      const items = menuItems(peerOneRow(), memberCtx());
      assert.deepEqual(keys(items), ["block", "exclude"]);
      assert.equal(items[0].label, "Block on this computer…");
      assert.ok(!items[0].disabled, "block is disabled");
      assert.equal(items[1].label, "Remove from network (admins only)");
      assert.equal(items[1].disabled, true);
    });

    it("an admin's row gives the same two items, no role item", () => {
      assert.deepEqual(keys(menuItems(peerOneRow({ admin: true }), memberCtx())), ["block", "exclude"]);
    });

    it("an offline member gives the same two items", () => {
      const row = peerTwoRow({ online: false, last_seen_sec: 9999999 });
      assert.deepEqual(keys(menuItems(row, memberCtx())), ["block", "exclude"]);
    });
  });

  describe("my own row, I am an admin", () => {
    it("offers rename, give up, leave, delete the network", () => {
      const items = menuItems(selfRow({ admin: true }), adminCtx());
      assert.deepEqual(keys(items), ["rename", "give-up", "leave", "delete-network"]);
      assert.deepEqual(items.map((i) => i.label), ["Rename", "Give up admin rights", "Leave network…", "Delete network for everyone…"]);
    });

    it("deleting the network is the red item", () => {
      const items = menuItems(selfRow({ admin: true }), adminCtx());
      assert.equal(items.find((i) => i.key === "delete-network").danger, true);
      assert.ok(!items.find((i) => i.key === "give-up").disabled);
    });

    it("the row is told from the context id, too", () => {
      const items = menuItems(peerOneRow({ admin: true }), adminCtx({ me: PEER_FULL }));
      assert.deepEqual(keys(items), ["rename", "give-up", "leave", "delete-network"]);
    });

    it("the only admin cannot give up the rights, and the label says why", () => {
      const items = menuItems(selfRow({ admin: true }), adminCtx({ onlyAdmin: true }));
      const item = items.find((i) => i.key === "give-up");
      assert.equal(item.disabled, true);
      assert.equal(item.label, "Give up admin rights (you're the only admin)");
      assert.deepEqual(keys(items), ["rename", "give-up", "leave", "delete-network"]);
    });

    it("leave and delete stay enabled for the only admin", () => {
      const items = menuItems(selfRow({ admin: true }), adminCtx({ onlyAdmin: true }));
      assert.ok(!items.find((i) => i.key === "leave").disabled);
      assert.ok(!items.find((i) => i.key === "delete-network").disabled);
    });
  });

  describe("my own row, I am a plain member", () => {
    it("offers rename and leave only", () => {
      const items = menuItems(selfRow({ admin: false }), memberCtx());
      assert.deepEqual(keys(items), ["rename", "leave"]);
      assert.deepEqual(items.map((i) => i.label), ["Rename", "Leave network…"]);
    });
  });

  describe("the rows of a managed network never offer voting items", () => {
    it("whatever the row, no withdraw, waiting, unadmit, forget or unforget", () => {
      const banned = ["withdraw", "waiting", "unadmit", "forget", "unforget"];
      const rows = [peerOneRow(), peerTwoRow({ online: false }), pendingRow(), peerOneRow({ vote: voteInfo() }), peerOneRow({ forget: forgetInfo() })];
      for (const ctx of [adminCtx(), memberCtx()]) {
        for (const row of rows) {
          const found = keys(menuItems(row, ctx)).filter((k) => banned.includes(k));
          assert.deepEqual(found, [], "voting items on " + JSON.stringify(row.status));
        }
      }
    });
  });
});

describe("isRoot", () => {
  describe("the network names its creator", () => {
    it("is true for the creator's id", () => {
      assert.equal(isRoot({ root: PEER_FULL }, PEER_FULL), true);
    });

    it("is false for another member", () => {
      assert.equal(isRoot({ root: PEER_FULL }, PEER2_FULL), false);
    });
  });

  describe("the network does not name a creator", () => {
    it("is false for everyone", () => {
      assert.equal(isRoot({ label: "work" }, PEER_FULL), false);
    });
  });

  describe("there is no network", () => {
    it("is false for null and undefined", () => {
      assert.equal(isRoot(null, PEER_FULL), false);
      assert.equal(isRoot(undefined, PEER_FULL), false);
    });
  });
});

describe("leaveKind", () => {
  describe("an admin who cannot leave", () => {
    it("asks to choose an admin first", () => {
      assert.equal(leaveKind({ meAdmin: true, canLeave: false }), "last-admin");
    });
  });

  describe("the only admin", () => {
    it("asks to choose an admin first", () => {
      assert.equal(leaveKind({ meAdmin: true, onlyAdmin: true }), "last-admin");
    });
  });

  describe("an admin among other admins", () => {
    it("leaves with the usual dialog", () => {
      assert.equal(leaveKind({ meAdmin: true, canLeave: true, onlyAdmin: false }), "leave");
      assert.equal(leaveKind({ meAdmin: true }), "leave");
    });
  });

  describe("a plain member", () => {
    it("leaves with the usual dialog", () => {
      assert.equal(leaveKind({ meAdmin: false, canLeave: true }), "leave");
      assert.equal(leaveKind({ meAdmin: false, onlyAdmin: false }), "leave");
    });
  });

  describe("a peer network", () => {
    it("leaves with the usual dialog", () => {
      assert.equal(leaveKind({}), "leave");
    });
  });
});

describe("votingHidden", () => {
  describe("a managed network", () => {
    it("hides the vote banner and the vote texts", () => {
      assert.equal(votingHidden({ kind: "Managed" }), true);
    });
  });

  describe("a peer network", () => {
    it("shows them", () => {
      assert.equal(votingHidden({ kind: "Peer" }), false);
    });
  });

  describe("an older node without a kind", () => {
    it("shows them", () => {
      assert.equal(votingHidden({}), false);
    });
  });

  describe("the list is not known yet", () => {
    it("shows them", () => {
      assert.equal(votingHidden(null), false);
      assert.equal(votingHidden(undefined), false);
    });
  });
});

const keys = (items) => items.map((i) => i.key);
const adminCtx = (over = {}) => ({ me: ME, managed: true, meAdmin: true, ...over });
const memberCtx = (over = {}) => ({ me: ME, managed: true, meAdmin: false, ...over });
