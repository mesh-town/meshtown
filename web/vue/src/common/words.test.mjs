/* `words.ts`: the label of a node without a name. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import { NET_NAME, EXIT_DOWN_TAIL, EXIT_VIA_HIM, NOT_RESPONDING_TEXT, agoText, anonymousPeerLabel, errReason, exitViaText, ROUTE_DIRECT_TEXT, routeViaText, failText, peerLabel, peerName, quotedNet, upperFirst } from "./words.ts";

describe("peerName", () => {
  it("has a name — the name", () => assert.equal(peerName({ id: "ab12", name: "peer-one" }), "peer-one"));
  it("empty or whitespace — \"member <id>\"", () => {
    assert.equal(peerName({ id: "ab12", name: "" }), "member ab12");
    assert.equal(peerName({ id: "ab12", name: "  " }), "member ab12");
    assert.equal(peerName({ id: "ab12" }), anonymousPeerLabel({ id: "ab12" }));
  });
});

describe("NET_NAME", () => {
  it("network name in texts", () => assert.equal(NET_NAME, "Mesh Town"));
});

describe("peerLabel", () => {
  const KITCHEN = { name: "kitchen", id: "ab12" };
  const HALL = { name: "hall", id: "cd34" };

  describe("no list of members passed", () => {
    it("with a name — the bare name, without an id", () => assert.equal(peerLabel(KITCHEN), "kitchen"));
    it("name of whitespace or null, no tag — bare id", () => {
      assert.equal(peerLabel({ name: "  ", id: "ab12" }), "ab12");
      assert.equal(peerLabel({ name: null, id: "ab12" }), "ab12");
    });
    it("name of whitespace or null, with tag — \"member <id>\"", () => {
      assert.equal(peerLabel({ name: "  ", id: "ab12" }, true), "member ab12");
      assert.equal(peerLabel({ name: null, id: "ab12" }, true), "member ab12");
    });
    it("with a name and a tag — the anonymous tag doesn't hide the name", () => {
      assert.equal(peerLabel(KITCHEN, true), "kitchen");
    });
  });

  describe("a list of members passed, all names differ", () => {
    it("the member itself in the list is not a duplicate — the bare name", () => {
      assert.equal(peerLabel(KITCHEN, false, [KITCHEN, HALL]), "kitchen");
    });
    it("a nameless member is still its bare id or \"member <id>\"", () => {
      const nameless = { name: "", id: "ef56" };
      assert.equal(peerLabel(nameless, false, [KITCHEN, nameless]), "ef56");
      assert.equal(peerLabel(nameless, true, [KITCHEN, nameless]), "member ef56");
    });
  });

  describe("another member in the list has the same name", () => {
    it("\"name (id)\" for each of the two", () => {
      const twin = { name: "kitchen", id: "ef56" };
      assert.equal(peerLabel(KITCHEN, false, [KITCHEN, twin, HALL]), "kitchen (ab12)");
      assert.equal(peerLabel(twin, false, [KITCHEN, twin, HALL]), "kitchen (ef56)");
    });
    it("case and spaces at the edges do not matter", () => {
      const twin = { name: "  KITCHEN ", id: "ef56" };
      assert.equal(peerLabel(KITCHEN, false, [KITCHEN, twin]), "kitchen (ab12)");
      assert.match(peerLabel(twin, false, [KITCHEN, twin]), /\(ef56\)$/);
    });
    it("the tag changes nothing for a named member", () => {
      assert.equal(peerLabel(KITCHEN, true, [KITCHEN, { name: "kitchen", id: "ef56" }]), "kitchen (ab12)");
    });
    it("a member not in the list itself (a route entry) is compared with the list by name", () => {
      assert.equal(peerLabel({ name: "kitchen", id: "ab12" }, false, [{ name: "kitchen", id: "zz99" }]), "kitchen (ab12)");
    });
  });
});

describe("quotedNet", () => {
  it("has a label — ' \"label\"' with a leading space", () => assert.equal(quotedNet("family"), ' "family"'));

  it("no label — empty, not ' \"null\"'", () => {
    for (const empty of [null, undefined, ""]) assert.equal(quotedNet(empty), "", String(empty));
  });
});

function apiError(message) {
  const e = new Error("HTTP 400");
  e.body = { message };
  return e;
}

describe("failText", () => {
  it("reason from body.message, first letter lowercase", () =>
    assert.equal(failText("save", apiError("Node is not responding")), "Couldn't save: node is not responding"));
  it("response with a code from the page — the program name is written as is", () =>
    assert.equal(failText("save", new ApiError(500, "boom", API_ERROR_TEXT.http(500))), "Couldn't save: " + NET_NAME + " responded with an error (code 500)"));
  it("abbreviation — case intact", () =>
    assert.equal(failText("save", apiError("DNS did not respond")), "Couldn't save: DNS did not respond"));
  it("reason from err.message", () => assert.equal(failText("sign in", new Error("Failure")), "Couldn't sign in: failure"));
  it("no err — tail with a hint", () => assert.equal(failText("sign in"), "Couldn't sign in — try again"));
  it("own hint — in the tail", () => assert.equal(failText("sign in", null, "check the network"), "Couldn't sign in — check the network"));
});

describe("agoText", () => {
  it("boundaries", () => {
    assert.equal(agoText(59), "just now");
    assert.equal(agoText(60), "1 min ago");
    assert.equal(agoText(3600), "1 h ago");
    assert.equal(agoText(86400 * 3), "3 d ago");
  });
  it("not a number — empty string", () => {
    assert.equal(agoText(null), "");
    assert.equal(agoText(undefined), "");
  });
});

describe("upperFirst and exitViaText", () => {
  it("uppercase first letter", () => assert.equal(upperFirst("couldn't find"), "Couldn't find"));
  it("empty — empty string", () => assert.equal(upperFirst(""), ""));
  it("the program name at the start is written as is", () => {
    assert.equal(upperFirst(NOT_RESPONDING_TEXT), "Mesh Town is not responding");
    assert.equal(upperFirst(NET_NAME + " didn't respond in time"), "Mesh Town didn't respond in time");
  });
  it("exit label", () => {
    assert.equal(exitViaText("Vasya"), "Exit via Vasya");
    assert.equal(EXIT_VIA_HIM, "Use as exit");
    assert.equal(ROUTE_DIRECT_TEXT, "In the browser, sites open directly");
    assert.equal(routeViaText("Vasya"), "In the browser, sites open via Vasya");
  });
});

describe("EXIT_DOWN_TAIL", () => {
  it("tail of the phrase \"<name> is offline — …\": true for any browser (Safari and Chromium on Linux go direct)", () => {
    assert.equal(EXIT_DOWN_TAIL, "sites may not open or may open directly");
  });

  it("does not promise that sites definitely won't open", () => {
    assert.ok(!EXIT_DOWN_TAIL.includes("sites don't open right now"), EXIT_DOWN_TAIL);
  });
});

describe("NOT_RESPONDING_TEXT", () => {
  it("names the program, not the word \"node\"", () => {
    assert.equal(NOT_RESPONDING_TEXT, "Mesh Town is not responding");
  });
});

describe("errReason", () => {
  it("the node's message from body wins over the error message", () => {
    assert.equal(errReason({ body: { message: " Node busy " }, message: "http 500" }), "Node busy");
  });
  it("no body.message — the error's message", () => assert.equal(errReason(new Error("Failure")), "Failure"));
  it("neither or not an error — empty string", () => {
    assert.equal(errReason({ body: {} }), "");
    assert.equal(errReason(null), "");
    assert.equal(errReason(undefined), "");
  });
});
