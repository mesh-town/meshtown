/* `nav.ts`: parsing the address hash and the list of shell sections. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NARROW_MORE, SECTIONS, SECTION_VIEWS, openableSection, parseHash, sectionViews, splitTabs } from "./nav.ts";

describe("parseHash", () => {
  it("a known section in the hash — its name, with the hash sign and without", () => {
    assert.equal(parseHash("#log"), "log");
    assert.equal(parseHash("home"), "home");
  });

  it("the old #overview link — \"Home\"", () => {
    assert.equal(parseHash("#overview"), "home");
  });

  it("a garbage, empty or missing hash — null", () => {
    assert.equal(parseHash("#bogus"), null, "garbage");
    assert.equal(parseHash(""), null, "empty");
    assert.equal(parseHash("#"), null, "a lone hash sign");
    assert.equal(parseHash(undefined), null, "no hash");
  });
});

describe("SECTION_VIEWS", () => {
  it("seven sections in tab order", () => {
    assert.deepEqual(SECTIONS, ["home", "peers", "domains", "ports", "network", "log", "settings"]);
    assert.deepEqual(SECTION_VIEWS.map((v) => v.id), SECTIONS);
  });

  it("tab labels are the old ones, as in the classic markup", () => {
    assert.deepEqual(SECTION_VIEWS.map((v) => v.tab), [
      "Home", "Members", "Site checks", "Ports", "Diagnostics", "Log", "Settings",
    ]);
  });

  it("the section title matches the tab label", () => {
    for (const v of SECTION_VIEWS) assert.equal(v.title, v.id === "home" ? null : v.tab, v.id);
  });
});

describe("sectionViews", () => {
  it("on a computer — all sections", () => assert.deepEqual(sectionViews(false), SECTION_VIEWS));

  it("in the phone app — without site checks and ports", () => {
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
  // OPEN QUESTION: the result shape `{ main, more }` (two lists of section views) is the likeliest reading of the plan.
  it("the \"More\" sections are ports, diagnostics and log", () => {
    assert.deepEqual(NARROW_MORE, ["ports", "network", "log"]);
  });

  it("on a computer — four main tabs and three under \"More\", in tab order", () => {
    const { main, more } = splitTabs(sectionViews(false));
    assert.deepEqual(main.map((v) => v.id), ["home", "peers", "domains", "settings"]);
    assert.deepEqual(more.map((v) => v.id), ["ports", "network", "log"]);
  });

  it("in the phone app — nothing is left for \"More\"", () => {
    const { main, more } = splitTabs(sectionViews(true));
    assert.deepEqual(main.map((v) => v.id), ["home", "peers", "settings"]);
    assert.deepEqual(more, []);
  });

  it("the views are the same objects, not copies", () => {
    const { more } = splitTabs(SECTION_VIEWS);
    assert.equal(more[0], SECTION_VIEWS.find((v) => v.id === "ports"));
  });
});
