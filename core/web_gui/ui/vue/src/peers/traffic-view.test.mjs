/* `bytesText(n)` — decimal sizes; `peerTrafficText(peer)` — the traffic line of a member row. */

import { describe, expect, it } from "vitest";
import { bytesText, peerTrafficText } from "./traffic-view.ts";

describe("bytesText", () => {
  it("formats decimal units", () => {
    expect(bytesText(0)).toBe("0 B");
    expect(bytesText(999)).toBe("999 B");
    expect(bytesText(1500)).toBe("1.5 KB");
    expect(bytesText(12_300_000)).toBe("12 MB");
    expect(bytesText(2_400_000_000)).toBe("2.4 GB");
  });
});

describe("peerTrafficText", () => {
  const row = (over) => ({ self: false, through_peer_bytes: 0, through_us_bytes: 0, ...over });

  it("is empty for two zeros and for the own row", () => {
    expect(peerTrafficText(row({}))).toBe("");
    expect(peerTrafficText(row({ self: true, through_peer_bytes: 5000 }))).toBe("");
    expect(peerTrafficText({ self: false })).toBe("");
  });

  it("shows one side", () => {
    expect(peerTrafficText(row({ through_peer_bytes: 1500 }))).toBe("You used their exit: 1.5 KB");
    expect(peerTrafficText(row({ through_us_bytes: 2000 }))).toBe("They used your exit: 2.0 KB");
  });

  it("shows both sides", () => {
    expect(peerTrafficText(row({ through_peer_bytes: 1500, through_us_bytes: 12_300_000 }))).toBe(
      "You used their exit: 1.5 KB · They used your exit: 12 MB",
    );
  });
});
