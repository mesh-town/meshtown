/* The banner on the tab of a network that was just added (`add-net-banner.ts`, `add-net-texts.ts`): one shared place for the
 * text, because the form that raises it may live on another page than the tab that shows it. */

import { describe, it, beforeEach } from "vitest";
import { reactive } from "vue";
import assert from "node:assert/strict";
import { bannerOutlived, useNetBanner, resetNetBanner, arrivedSlots, watchArrivals } from "./add-net-banner.ts";
import { createdBanner, joinedBanner } from "./add-net-texts.ts";
import { stayText } from "./add-net-actions.ts";

beforeEach(() => resetNetBanner());

describe("useNetBanner", () => {
  describe("nothing happened yet", () => {
    it("there is no banner", () => {
      assert.equal(useNetBanner().banner.value, null);
    });
  });

  describe("a network was added", () => {
    it("the banner holds the slot and the text", () => {
      const nb = useNetBanner();
      nb.show("k7q2xa", "You joined \"friends\".");
      assert.deepEqual({ ...nb.banner.value }, { slot: "k7q2xa", text: "You joined \"friends\"." });
    });

    it("another user of the banner sees it too", () => {
      useNetBanner().show("k7q2xa", "text");
      assert.equal(useNetBanner().banner.value?.slot, "k7q2xa");
    });

    it("a second network replaces the first banner", () => {
      const nb = useNetBanner();
      nb.show("k7q2xa", "first");
      nb.show("b3d5fg", "second");
      assert.deepEqual({ ...nb.banner.value }, { slot: "b3d5fg", text: "second" });
    });
  });

  describe("OK pressed", () => {
    it("clear removes the banner for everyone", () => {
      const nb = useNetBanner();
      nb.show("k7q2xa", "text");
      nb.clear();
      assert.equal(nb.banner.value, null);
      assert.equal(useNetBanner().banner.value, null);
    });

    it("clear with no banner changes nothing", () => {
      const nb = useNetBanner();
      nb.clear();
      assert.equal(nb.banner.value, null);
    });
  });

  describe("reset for a new test", () => {
    it("resetNetBanner drops a shown banner", () => {
      const nb = useNetBanner();
      nb.show("k7q2xa", "text");
      resetNetBanner();
      assert.equal(nb.banner.value, null);
    });
  });
});

describe("joinedBanner", () => {
  describe("no exit through a network member", () => {
    it("names the network, nothing else", () => {
      assert.equal(joinedBanner("friends", null), "You joined \"friends\".");
    });
  });

  describe("sites open through an exit", () => {
    it("adds the tail: the exit has not changed, with the peer and its network", () => {
      assert.equal(
        joinedBanner("friends", { peer: "Anna", net: "my devices" }),
        "You joined \"friends\". Your exit hasn't changed: sites still open via Anna (my devices).",
      );
    });
  });
});

describe("createdBanner", () => {
  it("says which network was created and where to invite from", () => {
    const text = createdBanner("family");
    assert.ok(text.startsWith("You created \"family\" and you're its admin."), text);
    assert.ok(text.includes("Invite to \"family\""), text);
  });
});

describe("stayText", () => {
  it("one network — its name", () => {
    assert.equal(stayText(["a"]), "You stay in \"a\". The new network gets its own tab in Networks.");
  });

  it("two networks — joined by \"and\"", () => {
    assert.equal(stayText(["a", "b"]), "You stay in \"a\" and \"b\". The new network gets its own tab in Networks.");
  });

  it("three networks — commas, then \"and\" before the last", () => {
    assert.equal(stayText(["a", "b", "c"]), "You stay in \"a\", \"b\" and \"c\". The new network gets its own tab in Networks.");
  });
});

describe("arrivedSlots", () => {
  describe("a network was being joined", () => {
    it("it now holds a key — it has arrived", () => {
      assert.deepEqual(arrivedSlots([net("k7q2xa", { joining: true })], [net("k7q2xa", { id_hex: "hex-k7" })]), ["k7q2xa"]);
    });

    it("it is still waiting — nothing has arrived", () => {
      const waiting = net("k7q2xa", { joining: true });
      assert.deepEqual(arrivedSlots([waiting], [waiting]), []);
    });

    it("it disappeared — nothing has arrived", () => {
      assert.deepEqual(arrivedSlots([net("k7q2xa", { joining: true })], []), []);
    });
  });

  describe("a network that already had a key", () => {
    it("it is not reported again", () => {
      const held = net("main", { id_hex: "hex-main" });
      assert.deepEqual(arrivedSlots([held], [held]), []);
    });
  });
});

describe("watchArrivals", () => {
  it("calls back once for a network whose key appeared", () => {
    const state = reactive({ nets: [net("main", { id_hex: "hex-main" }), net("k7q2xa", { joining: true })] });
    const arrived = [];
    watchArrivals(() => state.nets, (slot) => arrived.push(slot));
    state.nets = [net("main", { id_hex: "hex-main" }), net("k7q2xa", { id_hex: "hex-k7" })];
    assert.deepEqual(arrived, ["k7q2xa"]);
  });

  it("is quiet while the poll brings the same picture", () => {
    const state = reactive({ nets: [net("k7q2xa", { joining: true })] });
    const arrived = [];
    watchArrivals(() => state.nets, (slot) => arrived.push(slot));
    state.nets = [net("k7q2xa", { joining: true })];
    assert.deepEqual(arrived, []);
  });
});

function net(id, over = {}) {
  return { id, label: null, id_hex: null, joining: false, ...over };
}

describe("bannerOutlived", () => {
  const banner = { slot: "k7q2xa", text: "You created it." };
  const net = (id, peers_total) => ({ id, peers_total });

  it("is false without a banner", () => {
    assert.equal(bannerOutlived(null, [net("k7q2xa", 1)]), false);
  });
  it("is false while nobody else is in the network", () => {
    assert.equal(bannerOutlived(banner, [net("k7q2xa", 0)]), false);
  });
  it("is true once someone joined", () => {
    assert.equal(bannerOutlived(banner, [net("k7q2xa", 1)]), true);
  });
  it("is false when another network has a member", () => {
    assert.equal(bannerOutlived(banner, [net("k7q2xa", 0), net("m3n4pq", 1)]), false);
  });
  it("is false when the slot is not in the list", () => {
    assert.equal(bannerOutlived(banner, [net("m3n4pq", 3)]), false);
  });
});
