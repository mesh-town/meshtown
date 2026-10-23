/* `net-votes.ts`: requests waiting for this member's confirmation, per network (`slotVotes`, the circle on a network tab),
 * in total (`totalVotes`, the badge of the Members tab) and where the "requests to confirm" link leads (`votesTarget`).
 * `net-tabs-view.ts`: the tab row takes the circle from `slotVotes` and shows "no connection" for a network without a link. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { slotVotes, totalVotes, votesTarget } from "./net-votes.ts";
import { isDown, netTabs } from "./net-tabs-view.ts";
import { createStore } from "../core/store.ts";
import { baseState } from "../testing/section.mjs";
import { foreignVote, membersReply, pendingRow, peerOneRow, peerTwoRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";

const A = "main";
const B = "k7q2xa";

describe("slotVotes", () => {
  describe("a removal vote proposed by someone else", () => {
    it("counts one", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] } });
      assert.equal(slotVotes(store, A), 1);
    });
  });

  describe("a join request from someone else", () => {
    it("counts one", () => {
      const { store } = setup({ [A]: { rows: [pendingRow()] } });
      assert.equal(slotVotes(store, A), 1);
    });
  });

  describe("a removal vote and a join request together", () => {
    it("counts both", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm(), pendingRow()] } });
      assert.equal(slotVotes(store, A), 2);
    });
  });

  describe("requests that need nothing from me", () => {
    it("a removal vote I proposed is not counted", () => {
      const { store } = setup({ [A]: { rows: [peerOneRow({ vote: voteInfo() })] } });
      assert.equal(slotVotes(store, A), 0);
    });
    it("a removal vote I already joined is not counted", () => {
      const { store } = setup({ [A]: { rows: [peerOneRow({ vote: foreignVote({ mine: true }) })] } });
      assert.equal(slotVotes(store, A), 0);
    });
    it("a join request I already confirmed is not counted", () => {
      const { store } = setup({ [A]: { rows: [pendingRow({ admission: { votes: 1, need: 2, voters: [], mine: true, label: null } })] } });
      assert.equal(slotVotes(store, A), 0);
    });
    it("a list without requests gives zero", () => {
      const { store } = setup({});
      assert.equal(slotVotes(store, A), 0);
    });
  });

  describe("two networks", () => {
    it("each network counts only its own list", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm(), pendingRow()] } });
      assert.deepEqual([slotVotes(store, A), slotVotes(store, B)], [1, 2]);
    });
    it("requests of B do not leak into the number of A", () => {
      const { store } = setup({ [B]: { rows: [removalToConfirm(), pendingRow()] } });
      assert.equal(slotVotes(store, A), 0);
    });
  });

  describe("this computer is excluded from the network", () => {
    it("the list flag zeroes the number", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()], excluded: "members" } });
      assert.equal(slotVotes(store, A), 0);
    });
    it("the flag in the node state zeroes the number", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()], excluded: "state" } });
      assert.equal(slotVotes(store, A), 0);
    });
    it("only that network is zeroed, the other keeps its requests", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm()], excluded: "members" } });
      assert.deepEqual([slotVotes(store, A), slotVotes(store, B)], [1, 0]);
    });
  });

  describe("no data", () => {
    it("an unknown network gives zero", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] } });
      assert.equal(slotVotes(store, "zzzzzz"), 0);
    });
    it("a network whose member list has not arrived gives zero", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: null } });
      assert.equal(slotVotes(store, B), 0);
    });
  });
});

describe("totalVotes", () => {
  describe("one request in A and two in B", () => {
    it("the badge shows three", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm(), pendingRow()] } });
      assert.equal(totalVotes(store), 3);
    });
  });

  describe("this computer is excluded from B", () => {
    it("B adds nothing to the sum", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm(), pendingRow()], excluded: "members" } });
      assert.equal(totalVotes(store), 1);
    });
  });

  describe("a network that is not visible", () => {
    it("its requests are not in the sum", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm()], keyless: true } });
      assert.equal(totalVotes(store), 1);
    });
  });

  describe("no requests anywhere", () => {
    it("the badge shows zero", () => {
      const { store } = setup({ [A]: {}, [B]: {} });
      assert.equal(totalVotes(store), 0);
    });
  });

  describe("a store without networks", () => {
    it("the badge shows zero", () => {
      assert.equal(totalVotes(createStore()), 0);
    });
  });
});

describe("votesTarget", () => {
  describe("two networks, requests only in B", () => {
    it("leads to the tab of B", () => {
      const { store } = setup({ [A]: {}, [B]: { rows: [removalToConfirm()] } });
      assert.equal(votesTarget(store), "#peers/k7q2xa");
    });
  });

  describe("two networks, requests only in A", () => {
    it("leads to the tab of A", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: {} });
      assert.equal(votesTarget(store), "#peers/main");
    });
  });

  describe("two networks, requests in both", () => {
    it("leads to the Members page as a whole", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [pendingRow()] } });
      assert.equal(votesTarget(store), "#peers");
    });
  });

  describe("two networks, requests only in B but this computer is excluded from it", () => {
    it("leads to the Members page as a whole", () => {
      const { store } = setup({ [A]: {}, [B]: { rows: [removalToConfirm()], excluded: "members" } });
      assert.equal(votesTarget(store), "#peers");
    });
  });

  describe("one network with requests", () => {
    it("leads to the Members page as a whole", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] } });
      assert.equal(votesTarget(store), "#peers");
    });
  });

  describe("no requests", () => {
    it("leads to the Members page as a whole", () => {
      const { store } = setup({ [A]: {}, [B]: {} });
      assert.equal(votesTarget(store), "#peers");
    });
  });
});

describe("netTabs", () => {
  describe("the circle of a tab", () => {
    it("equals slotVotes of that network", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()] }, [B]: { rows: [removalToConfirm(), pendingRow()] } });
      assert.deepEqual(netTabs(store).map((t) => t.votes), [1, 2]);
    });
    it("ignores members_voting of the brief", () => {
      const { store } = setup({ [A]: { brief: { members_voting: 5 } } });
      assert.equal(netTabs(store)[0].votes, 0);
    });
    it("is zero when this computer is excluded, though the brief counts requests", () => {
      const { store } = setup({ [A]: {}, [B]: { rows: [removalToConfirm()], excluded: "members", brief: { members_voting: 1 } } });
      assert.equal(netTabs(store)[1].votes, 0);
    });
  });

  describe("a network with members, none of them online", () => {
    it("status offline: the tail is no connection", () => {
      const { store } = setup({ [A]: { brief: { status: "offline", peers_online: 0, peers_total: 2 } }, [B]: {} });
      assert.equal(netTabs(store)[0].sub, "· no connection");
    });
    it("status offline: the hover title says no connection", () => {
      const { store } = setup({ [A]: { brief: { status: "offline", peers_online: 0, peers_total: 2 } }, [B]: {} });
      assert.equal(netTabs(store)[0].title, "friends — no connection");
    });
    it("the circle of requests stays on the tab", () => {
      const { store } = setup({ [A]: { rows: [removalToConfirm()], brief: { status: "offline", peers_online: 0, peers_total: 2 } }, [B]: {} });
      assert.equal(netTabs(store)[0].votes, 1);
    });
  });

  describe("a broken network", () => {
    it("shows no connection even when peers are counted online", () => {
      const { store } = setup({ [A]: { brief: { status: "broken", peers_online: 2, peers_total: 2 } }, [B]: {} });
      assert.equal(netTabs(store)[0].sub, "· no connection");
    });
  });

  describe("a healthy network", () => {
    it("shows the online count", () => {
      const { store } = setup({ [A]: { brief: { peers_online: 2, peers_total: 3 } }, [B]: {} });
      assert.equal(netTabs(store)[0].sub, "· 2 online");
    });
  });

  describe("a network with nobody else in it", () => {
    it("is not down: zero of zero online", () => {
      const { store } = setup({ [A]: { brief: { peers_online: 0, peers_total: 0 } }, [B]: {} });
      assert.equal(netTabs(store)[0].sub, "· 0 online");
    });
  });
});

describe("isDown", () => {
  it("broken is down", () => {
    assert.equal(isDown(brief(A, { status: "broken", peers_online: 1, peers_total: 1 })), true);
  });
  it("members known and none online is down", () => {
    assert.equal(isDown(brief(A, { status: "offline", peers_online: 0, peers_total: 3 })), true);
  });
  it("some member online is not down", () => {
    assert.equal(isDown(brief(A, { peers_online: 1, peers_total: 3 })), false);
  });
  it("no members at all is not down", () => {
    assert.equal(isDown(brief(A, { peers_online: 0, peers_total: 0 })), false);
  });
});

// A removal vote proposed by peer-one, which this node ("You") has not joined yet.
const removalToConfirm = () => peerTwoRow({ vote: foreignVote() });

function brief(id, over = {}) {
  return {
    id, label: id === A ? "friends" : "my devices", id_hex: "00" + id, status: "running", exit_enabled: false, proxy: id === A,
    route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: 1, peers_total: 1, ...over,
  };
}

// A page store with the given networks (in the order of the keys). Spec of a network: `rows` — rows of its member list
// (omitted: only "You"; `null`: the list has not arrived), `excluded` — where the "excluded" flag is set (`"members"` or `"state"`),
// `keyless` — the network holds no key, so it is not visible, `brief` — fields of its brief.
function setup(specs) {
  const store = createStore();
  for (const [id, spec] of Object.entries(specs)) {
    const b = brief(id, { id_hex: spec.keyless ? null : "00" + id, ...(spec.brief ?? {}) });
    const state = baseState(spec.excluded === "state" ? { members: { me_excluded: true } } : {});
    const members = spec.rows === null ? null : membersReply({ rows: [selfRow(), ...(spec.rows ?? [])], me_excluded: spec.excluded === "members" });
    store.nets.push(b);
    store.slots[id] = { id, brief: b, state, members, config: null, configRev: null, configMeta: null, bootId: null };
    if (id === store.lead) Object.assign(store, { state, members });
  }
  return { store };
}
