/* Ring view for the panel: `ringView` assembles members, center and legend; `useRingSize` takes the size by window width. */

import { describe, expect, it, vi } from "vitest";
import { unref } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { ringView, useRingSize } from "./ring-view.ts";
import { RING } from "./ring-config.ts";

describe("ringView", () => {
  it("no snapshot — null", () => {
    expect(ringView(null, "green")).toBeNull();
  });

  it("members match ringMembers: the exit first, label from the name", () => {
    const v = ringView(snap(), "green");
    expect(v.members.map((m) => [m.role, m.label])).toEqual([["via", "Exit"], ["online", "aa"], ["offline", "zz"]]);
  });

  it("center — \"N of M online\"", () => {
    expect(ringView(snap(), "green").center).toEqual({ main: "2 of 3", sub: "online" });
  });

  it("legend — by roles with members", () => {
    expect(ringView(snap(), "green").legend).toEqual([
      { role: "via", text: "Your exit: Exit" },
      { role: "online", text: "Also online: 1" },
      { role: "offline", text: "Offline: 1" },
    ]);
  });

  it("without members: green — \"only you\", red — \"no network\", legend is empty", () => {
    const empty = { peers: [], route: { state: "direct", exits: [] }, exit: { clients: [] } };
    expect(ringView(empty, "green").center).toEqual({ main: "only you", sub: "" });
    expect(ringView(empty, "red").center).toEqual({ main: "no network", sub: "" });
    expect(ringView(empty, "green").legend).toEqual([]);
  });
});

describe("useRingSize", () => {
  describe("narrow window (max-width: 599px)", () => {
    it("size and thickness are narrow", () => {
      const mq = stubMatchMedia(true);
      const s = readSize();
      expect(mq).toHaveBeenCalledWith("(max-width: 599px)");
      expect(s).toEqual({ size: RING.sizeNarrowPx, thickness: RING.thickNarrowPx });
    });
  });

  describe("wide window", () => {
    it("size and thickness are wide", () => {
      stubMatchMedia(false);
      expect(readSize()).toEqual({ size: RING.sizeWidePx, thickness: RING.thickWidePx });
    });
  });

  describe("no matchMedia", () => {
    it("wide sizes are taken", () => {
      vi.stubGlobal("matchMedia", undefined);
      expect(readSize()).toEqual({ size: RING.sizeWidePx, thickness: RING.thickWidePx });
    });
  });
});

/** What `useRingSize` returned, with refs unwrapped — so the test does not depend on whether it is a ref or a value. */
function readSize() {
  const w = withSetup(() => useRingSize());
  const r = unref(w.result);
  const out = { size: unref(r.size), thickness: unref(r.thickness) };
  w.unmount();
  return out;
}

function stubMatchMedia(matches) {
  const mq = vi.fn((media) => ({
    matches, media,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {},
  }));
  vi.stubGlobal("matchMedia", mq);
  return mq;
}

/** The exit "Exit" (active, in peers), one online, one not. */
function snap() {
  const p = (id, extra = {}) => ({ id, id_full: `${id}_full`, name: null, self: false, online: true, ...extra });
  return {
    peers: [p("me", { self: true }), p("ex", { name: "Exit" }), p("aa"), p("zz", { online: false })],
    route: { state: "exit", exits: [{ id: "ex", id_full: "ex_full", name: "Exit", online: true, active: true }] },
    exit: { enabled: false, domains: 0, clients: [] },
  };
}
