/* Ring texts: the center, the legend, the arc tooltip — the exact words. */

import { describe, expect, it } from "vitest";
import { arcTitle, ringCenter, ringLegend } from "./ring-text.ts";

describe("ringCenter", () => {
  describe("no members", () => {
    it("green level — \"only you\"", () => {
      expect(ringCenter([], "green")).toEqual({ main: "only you", sub: "" });
    });

    it("red level — \"no network\"", () => {
      expect(ringCenter([], "red")).toEqual({ main: "no network", sub: "" });
    });

    it("grey level — \"no network\"", () => {
      expect(ringCenter([], "grey")).toEqual({ main: "no network", sub: "" });
    });
  });

  describe("members present", () => {
    it("\"N of M\": N not offline, M all members", () => {
      const m = members({ via: 1, client: 2, online: 3, offline: 4 });
      expect(ringCenter(m, "green")).toEqual({ main: "6 of 10", sub: "online" });
    });

    it("all offline — \"0 of 3\"", () => {
      expect(ringCenter(members({ offline: 3 }), "green")).toEqual({ main: "0 of 3", sub: "online" });
    });

    it("the level does not affect the text", () => {
      const m = members({ online: 2, offline: 1 });
      expect(ringCenter(m, "red")).toEqual(ringCenter(m, "green"));
      expect(ringCenter(m, "grey")).toEqual(ringCenter(m, "green"));
    });
  });
});

describe("ringLegend", () => {
  it("no members — empty", () => {
    expect(ringLegend([])).toEqual([]);
  });

  it("only roles with members, in the order via, client, online, offline", () => {
    const m = members({ offline: 2, via: 1, online: 3 });
    expect(ringLegend(m).map((l) => l.role)).toEqual(["via", "online", "offline"]);
  });

  it("texts of all roles verbatim", () => {
    const m = [
      { role: "via", label: "Dad-PC", idFull: "v" },
      ...members({ client: 5, online: 28, offline: 6 }),
    ];
    expect(ringLegend(m)).toEqual([
      { role: "via", text: "Your exit: Dad-PC" },
      { role: "client", text: "Opening sites through you: 5" },
      { role: "online", text: "Also online: 28" },
      { role: "offline", text: "Offline: 6" },
    ]);
  });

  it("no member with the role via — the online group is \"Online: N\"", () => {
    const m = members({ client: 5, online: 28, offline: 6 });
    expect(ringLegend(m).find((l) => l.role === "online").text).toBe("Online: 28");
  });
});

describe("arcTitle", () => {
  describe("arc of one member", () => {
    it("via", () => {
      expect(arcTitle(arc("via", ["Sam"]))).toBe("Sam — your exit, online");
    });

    it("client", () => {
      expect(arcTitle(arc("client", ["Sam"]))).toBe("Sam — opens sites through you");
    });

    it("online", () => {
      expect(arcTitle(arc("online", ["Sam"]))).toBe("Sam — online");
    });

    it("offline", () => {
      expect(arcTitle(arc("offline", ["Sam"]))).toBe("Sam — offline");
    });
  });

  describe("arc of several members", () => {
    it("online — \"Online: count\"", () => {
      expect(arcTitle(arc("online", names(28)))).toBe("Online: 28");
    });

    it("client — \"Opening sites through you: count\"", () => {
      expect(arcTitle(arc("client", names(5)))).toBe("Opening sites through you: 5");
    });

    it("offline — \"Offline: count\"", () => {
      expect(arcTitle(arc("offline", names(6)))).toBe("Offline: 6");
    });

    it("via — \"Exits: count\"", () => {
      expect(arcTitle(arc("via", names(2)))).toBe("Exits: 2");
    });
  });
});

function names(n) {
  return Array.from({ length: n }, (_, i) => `n${i}`);
}

function arc(role, labels) {
  const members = labels.map((label) => ({ role, label, idFull: `${label}_full` }));
  return { role, members, startRad: 0, endRad: 1, lengthPx: 10 };
}

/** Members by role: `members({ online: 3 })`. */
function members(counts) {
  return Object.entries(counts).flatMap(([role, n]) =>
    Array.from({ length: n }, (_, i) => ({ role, label: `${role}-${i}`, idFull: `${role}-${i}_full` })),
  );
}
