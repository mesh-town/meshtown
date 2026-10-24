/* The tab row for Managed networks: `work · managed · 3 online` (the tab's `sub` is `· managed · 3 online`). A Peer tab is
 * unchanged. A tab whose network has a deletion notice (same `id_hex`) shows `· deleted` instead of the online count. */

import { describe, expect, it } from "vitest";
import { MAIN, setup } from "../core/overview-test-support.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { netTabs } from "./net-tabs-view.ts";

const NOTICE = { id: 3, kind: "net_deleted", label: "work", id_hex: "9c0100", by: "b".repeat(64), by_name: "anna", at_unix: 1792400000 };

describe("a Managed tab", () => {
  it("says managed and the online count", () => {
    const tab = tabOf({ kind: "Managed", peers_online: 3, peers_total: 4 });
    expect(tab.sub).toBe("· managed · 3 online");
  });

  it("with nobody online says 0 online after managed", () => {
    const tab = tabOf({ kind: "Managed", peers_online: 0, peers_total: 0 });
    expect(tab.sub).toBe("· managed · 0 online");
  });

  it("with no connection keeps the no-connection text", () => {
    const peer = tabOf({ peers_online: 0, peers_total: 2 });
    const managed = tabOf({ kind: "Managed", peers_online: 0, peers_total: 2 });
    expect(managed.sub).toBe(peer.sub);
  });
});

describe("a Peer tab", () => {
  it("is as before", () => {
    expect(tabOf({ kind: "Peer", peers_online: 3, peers_total: 4 }).sub).toBe("· 3 online");
  });

  it("without a kind is as before", () => {
    expect(tabOf({ peers_online: 3, peers_total: 4 }).sub).toBe("· 3 online");
  });
});

describe("a tab with a deletion notice of the same network", () => {
  it("says deleted instead of the online count", () => {
    const tab = tabOf({ kind: "Managed", peers_online: 3, peers_total: 4 }, [NOTICE]);
    expect(tab.sub).toBe("· deleted");
  });

  it("a notice of another network changes nothing", () => {
    const tab = tabOf({ kind: "Managed", peers_online: 3, peers_total: 4 }, [{ ...NOTICE, id_hex: "other" }]);
    expect(tab.sub).toBe("· managed · 3 online");
  });
});

describe("a tab of a network that removed this computer from the member list", () => {
  const removed = (over = {}) => membersReply({ rows: [selfRow()], me_removed: true, ...over });

  it("says removed instead of the online count", () => {
    expect(tabOf({ kind: "Managed", peers_online: 3, peers_total: 4 }, [], removed()).sub).toBe("· removed");
  });

  it("does the same for a Peer network", () => {
    expect(tabOf({ peers_online: 3, peers_total: 4 }, [], removed()).sub).toBe("· removed");
  });

  it("a deletion notice wins over it", () => {
    expect(tabOf({ kind: "Managed", peers_online: 3, peers_total: 4 }, [NOTICE], removed()).sub).toBe("· deleted");
  });

  it("a list without the flag changes nothing", () => {
    expect(tabOf({ peers_online: 3, peers_total: 4 }, [], removed({ me_removed: false })).sub).toBe("· 3 online");
  });
});

function tabOf(brief, notices = [], members = null) {
  const { store } = setup([{ id: MAIN, brief: { label: "work", id_hex: "9c0100", ...brief } }]);
  store.notices = notices;
  store.members = members;
  return netTabs(store)[0];
}
