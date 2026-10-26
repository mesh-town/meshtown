/* `nav.ts`: parsing the address hash and the list of shell sections. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  NARROW_MORE, SECTIONS, SECTION_VIEWS, openableSection, parseHash, peersHash, peersTail, pickPeersTab, sectionViews, splitTabs,
} from "./nav.ts";

describe("parseHash", () => {
  it("a known section in the hash — its name, with the hash sign and without", () => {
    assert.equal(parseHash("#log"), "log");
    assert.equal(parseHash("home"), "home");
  });

  it("the old #overview link — \"Home\"", () => {
    assert.equal(parseHash("#overview"), "home");
  });

  it("#ports → peers", () => {
    assert.equal(parseHash("#ports"), "peers");
    assert.equal(parseHash("ports"), "peers", "without the hash sign");
  });

  it("a garbage, empty or missing hash — null", () => {
    assert.equal(parseHash("#bogus"), null, "garbage");
    assert.equal(parseHash(""), null, "empty");
    assert.equal(parseHash("#"), null, "a lone hash sign");
    assert.equal(parseHash(undefined), null, "no hash");
  });
});

describe("SECTION_VIEWS", () => {
  it("six sections in tab order, ports is not one of them", () => {
    assert.deepEqual(SECTIONS, ["home", "peers", "domains", "network", "log", "settings"]);
    assert.deepEqual(SECTION_VIEWS.map((v) => v.id), SECTIONS);
  });

  it("tab labels: the peers section is called Networks", () => {
    assert.deepEqual(SECTION_VIEWS.map((v) => v.tab), [
      "Home", "Networks", "Site checks", "Diagnostics", "Log", "Settings",
    ]);
  });

  it("the section title matches the tab label", () => {
    for (const v of SECTION_VIEWS) assert.equal(v.title, v.id === "home" ? null : v.tab, v.id);
  });
});

describe("sectionViews", () => {
  it("on a computer — all sections", () => assert.deepEqual(sectionViews(false), SECTION_VIEWS));

  it("in the phone app — without site checks, diagnostics and log", () => {
    assert.deepEqual(sectionViews(true).map((v) => v.id), ["home", "peers", "settings"]);
  });
});

describe("openableSection", () => {
  it("a computer opens any known section", () => assert.equal(openableSection("#log", false), "log"));
  it("the phone app sends a hidden section to Home", () => assert.equal(openableSection("#log", true), "home"));
  it("the phone app keeps a visible section", () => assert.equal(openableSection("#peers", true), "peers"));
  it("garbage goes to Home", () => assert.equal(openableSection("#bogus", false), "home"));
});

describe("splitTabs", () => {
  it("the \"More\" sections are diagnostics and log", () => {
    assert.deepEqual(NARROW_MORE, ["network", "log"]);
  });

  it("on a computer — four main tabs and two under \"More\", in tab order", () => {
    const { main, more } = splitTabs(sectionViews(false));
    assert.deepEqual(main.map((v) => v.id), ["home", "peers", "domains", "settings"]);
    assert.deepEqual(more.map((v) => v.id), ["network", "log"]);
  });

  it("in the phone app — nothing is left for \"More\"", () => {
    const { main, more } = splitTabs(sectionViews(true));
    assert.deepEqual(main.map((v) => v.id), ["home", "peers", "settings"]);
    assert.deepEqual(more, []);
  });

  it("the views are the same objects, not copies", () => {
    const { more } = splitTabs(SECTION_VIEWS);
    assert.equal(more[0], SECTION_VIEWS.find((v) => v.id === "network"));
  });
});

describe("peers tab in the hash", () => {
  const NET_B = "k7q2xa";

  describe("parseHash", () => {
    it("#peers with a tail of any kind is still the members section", () => {
      assert.equal(parseHash(`#peers/${NET_B}`), "peers");
      assert.equal(parseHash("#peers/add"), "peers");
      assert.equal(parseHash("#peers/anything-at-all"), "peers");
      assert.equal(parseHash("#peers/"), "peers", "an empty tail");
    });

    it("a tail on any other section is not a section", () => {
      assert.equal(parseHash("#home/x"), null);
      assert.equal(parseHash("#log/main"), null);
    });

    it("the plain section hashes are unchanged", () => {
      assert.equal(parseHash("#peers"), "peers");
      assert.equal(parseHash("#overview"), "home");
    });
  });

  describe("openableSection", () => {
    it("#ports opens the networks section, on a computer and in the phone app", () => {
      assert.equal(openableSection("#ports", false), "peers");
      assert.equal(openableSection("#ports", true), "peers");
    });

    it("a hash with a net tail opens the members section", () => {
      assert.equal(openableSection(`#peers/${NET_B}`, false), "peers");
    });

    it("in the phone app the members section with a tail stays visible", () => {
      assert.equal(openableSection("#peers/add", true), "peers");
    });
  });

  describe("peersTail", () => {
    it("main, a six-letter net id and add are tails", () => {
      assert.equal(peersTail("#peers/main"), "main");
      assert.equal(peersTail(`#peers/${NET_B}`), NET_B);
      assert.equal(peersTail("#peers/add"), "add");
    });

    it("a well-formed but unknown id is still a tail (the check is syntactic)", () => {
      assert.equal(peersTail("#peers/zzzzzz"), "zzzzzz");
    });

    it("no tail — null", () => {
      assert.equal(peersTail("#peers"), null, "bare section");
      assert.equal(peersTail("#peers/"), null, "empty tail");
      assert.equal(peersTail(""), null, "empty hash");
      assert.equal(peersTail(undefined), null, "no hash");
    });

    it("a malformed tail — null", () => {
      assert.equal(peersTail("#peers/K7Q2XA"), null, "upper case");
      assert.equal(peersTail("#peers/k7q2x"), null, "five characters");
      assert.equal(peersTail("#peers/k7q2xab"), null, "seven characters");
      assert.equal(peersTail("#peers/k7q2x1"), null, "a digit outside 2-7");
      assert.equal(peersTail("#peers/k7q2xa/extra"), null, "a second segment");
    });

    it("a subsection after the net still gives the net", () => {
      assert.equal(peersTail("#peers/main/services"), "main");
      assert.equal(peersTail(`#peers/${NET_B}/device`), NET_B);
      assert.equal(peersTail(`#peers/${NET_B}/members`), NET_B);
    });

    it("an unknown subsection — null", () => {
      assert.equal(peersTail(`#peers/${NET_B}/bogus`), null);
      assert.equal(peersTail(`#peers/${NET_B}/services/more`), null);
    });

    it("a tail on another section — null", () => {
      assert.equal(peersTail("#home"), null);
      assert.equal(peersTail(`#home/${NET_B}`), null);
      assert.equal(peersTail(`#log/${NET_B}`), null);
    });
  });

  describe("peersHash", () => {
    it("no tab (one net) — the bare section, no tail", () => {
      assert.equal(peersHash(null), "#peers");
    });

    it("a tab — the section with the tab as the tail", () => {
      assert.equal(peersHash(NET_B), `#peers/${NET_B}`);
      assert.equal(peersHash("main"), "#peers/main");
      assert.equal(peersHash("add"), "#peers/add");
    });

    it("members is the default subsection and writes no tail", () => {
      assert.equal(peersHash(NET_B, "members"), `#peers/${NET_B}`);
      assert.equal(peersHash("main", "members"), "#peers/main");
    });

    it("services and device are written after the net", () => {
      assert.equal(peersHash(NET_B, "services"), `#peers/${NET_B}/services`);
      assert.equal(peersHash("main", "device"), "#peers/main/device");
    });

    it("no tab — the bare section, whatever the subsection", () => {
      assert.equal(peersHash(null, "services"), "#peers");
    });

    it("add has no subsection", () => {
      assert.equal(peersHash("add"), "#peers/add");
    });

    it("what peersHash writes, peersTail and parseHash read back", () => {
      for (const t of ["main", NET_B, "add"]) {
        assert.equal(peersTail(peersHash(t)), t, t);
        assert.equal(parseHash(peersHash(t)), "peers", t);
      }
      assert.equal(peersTail(peersHash(null)), null);
    });

    it("the net survives every subsection round trip", () => {
      for (const sub of ["members", "services", "device"]) {
        assert.equal(peersTail(peersHash(NET_B, sub)), NET_B, sub);
        assert.equal(parseHash(peersHash(NET_B, sub)), "peers", sub);
      }
    });
  });

  describe("pickPeersTab", () => {
    const visible = ["main", NET_B];

    it("add is always kept, even when it is not a visible slot", () => {
      assert.equal(pickPeersTab("add", visible), "add");
      assert.equal(pickPeersTab("add", []), "add");
    });

    it("a wanted visible slot is picked", () => {
      assert.equal(pickPeersTab(NET_B, visible), NET_B);
    });

    it("an unknown or hidden slot falls back to the first visible", () => {
      assert.equal(pickPeersTab("zzzzzz", visible), "main");
      assert.equal(pickPeersTab("zzzzzz", [NET_B, "main"]), NET_B, "the first in the list, not main");
    });

    it("nothing wanted — the first visible", () => {
      assert.equal(pickPeersTab(null, visible), "main");
    });

    it("no visible slots and nothing to keep — null", () => {
      assert.equal(pickPeersTab(null, []), null);
      assert.equal(pickPeersTab("zzzzzz", []), null);
    });
  });
});
