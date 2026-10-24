/* The Managed extras of the overview: `nets[].kind`, `nets[].me_admin`, `nets[].can_invite` and the top-level `notices[]` survive
 * parsing and land in the store (`Store.notices`, `[]` by default). A node without these fields (older, or Peer only) leaves them off. */

import { describe, expect, it } from "vitest";
import { acceptOverview, legacyOverview, parseOverview } from "./overview.ts";
import { createStore } from "./store.ts";
import { MAIN, overviewOf, rawOverview, stateOf } from "./overview-test-support.mjs";

const NOTICE = { id: 7, kind: "net_deleted", label: "family", id_hex: "dead0001", by: "b".repeat(64), by_name: "anna", at_unix: 1792400000 };
const MANAGED_BRIEF = { kind: "Managed", me_admin: false, can_invite: false };

describe("parseOverview", () => {
  describe("a brief with the Managed fields", () => {
    it("keeps kind, me_admin and can_invite", () => {
      const raw = rawOverview();
      raw.nets[0] = { ...raw.nets[0], ...MANAGED_BRIEF };
      const brief = parseOverview(raw).nets[0];
      expect(brief.kind).toBe("Managed");
      expect(brief.me_admin).toBe(false);
      expect(brief.can_invite).toBe(false);
    });
  });

  describe("a brief of a Peer-only node", () => {
    it("has no kind, so nothing reads as Managed", () => {
      const brief = parseOverview(rawOverview()).nets[0];
      expect(brief.kind).toBeUndefined();
      expect(brief.can_invite).toBeUndefined();
    });
  });

  describe("notices", () => {
    it("are kept as the node lists them", () => {
      expect(parseOverview(rawOverview({ notices: [NOTICE] })).notices).toEqual([NOTICE]);
    });

    it("missing in an older node means none", () => {
      expect(parseOverview(rawOverview()).notices ?? []).toEqual([]);
    });
  });
});

describe("acceptOverview", () => {
  it("puts the notices into the store", () => {
    const store = createStore();
    acceptOverview(store, { ...overviewOf([{ id: MAIN }]), notices: [NOTICE] });
    expect(store.notices).toEqual([NOTICE]);
  });

  it("replaces them with the next poll, so an acknowledged notice goes away", () => {
    const store = createStore();
    acceptOverview(store, { ...overviewOf([{ id: MAIN }]), notices: [NOTICE] });
    acceptOverview(store, { ...overviewOf([{ id: MAIN }]), notices: [] });
    expect(store.notices).toEqual([]);
  });

  it("an overview without the field leaves an empty list, not undefined", () => {
    const store = createStore();
    acceptOverview(store, overviewOf([{ id: MAIN }]));
    expect(store.notices).toEqual([]);
  });

  it("keeps kind, me_admin and can_invite in the slice brief", () => {
    const store = createStore();
    const ov = overviewOf([{ id: MAIN, brief: MANAGED_BRIEF }]);
    acceptOverview(store, ov);
    expect(store.slots[MAIN].brief).toMatchObject(MANAGED_BRIEF);
  });
});

describe("the store", () => {
  it("starts with no notices", () => {
    expect(createStore().notices).toEqual([]);
  });
});

describe("a node without /api/overview (legacy)", () => {
  it("takes the notices from the snapshot", () => {
    const state = stateOf(MAIN, { notices: [NOTICE] });
    expect(legacyOverview(state, null).notices).toEqual([NOTICE]);
  });

  it("takes the kind and the invite right from network of the snapshot", () => {
    const state = stateOf(MAIN, { network: { id_hex: "net-aaaa", kind: "Managed", me_role: "member", can_invite: false } });
    const brief = legacyOverview(state, null).nets[0];
    expect(brief.kind).toBe("Managed");
    expect(brief.can_invite).toBe(false);
    expect(brief.me_admin).toBe(false);
  });
});
