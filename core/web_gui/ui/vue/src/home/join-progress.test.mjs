/* Progress of joining by invite (`join-progress.ts`): snapshot and "join sent" → phase, text and spinner. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { joinProgress, SEARCH_SUB } from "./join-progress.ts";

const info = (over) => ({ state: "waiting", message: "waiting", label: null, expires_unix: 1, ...over });
const snap = (over = {}, node = {}) => ({ boot_id: "b1", node: { status: "online", ...node }, network: null, ...over });

describe("joinProgress", () => {
  it("nothing in progress — null", () => {
    assert.equal(joinProgress(null), null);
    assert.equal(joinProgress(snap()), null);
    assert.equal(joinProgress(snap({}, { invite_join: null })), null);
  });

  it("waiting without a name: \"Looking for the network…\" with a hint", () => {
    const p = joinProgress(snap({}, { invite_join: info() }));
    assert.deepEqual(p, { phase: "searching", main: "Looking for the network…", sub: SEARCH_SUB });
  });

  it("waiting with a network name — the name in quotes", () => {
    const p = joinProgress(snap({}, { invite_join: info({ label: "family" }) }));
    assert.equal(p.main, "Looking for the network \"family\"…");
  });

  it("refused — the node message verbatim, no hint", () => {
    const p = joinProgress(snap({}, { invite_join: info({ state: "refused", message: "Already used." }) }));
    assert.deepEqual(p, { phase: "refused", main: "Already used.", sub: "" });
  });

  it("join sent, same start — \"Joining the network…\"", () => {
    const p = joinProgress(snap(), { bootIdAtSend: "b1" });
    assert.equal(p.phase, "entering");
    assert.equal(p.main, "Joining the network…");
  });

  it("join sent, no snapshots (absence) or the node is starting — still joining", () => {
    assert.equal(joinProgress(snap({ boot_id: "b2" }), { bootIdAtSend: "b1" }, "silent").phase, "entering");
    assert.equal(joinProgress(snap({ boot_id: "b2" }, { status: "starting" }), { bootIdAtSend: "b1" }).phase, "entering");
    assert.equal(joinProgress(null, { bootIdAtSend: null }).phase, "entering");
  });

  it("node answered with a new start — null; the name from the snapshot's network", () => {
    assert.equal(joinProgress(snap({ boot_id: "b2" }), { bootIdAtSend: "b1" }), null);
    const p = joinProgress(snap({ network: { label: "home" } }), { bootIdAtSend: "b1" });
    assert.equal(p.main, "Joining the network \"home\"…");
  });

  it("response \"waiting for members\", no snapshot with invite_join yet — straight to \"Looking\", not \"Joining\"", () => {
    assert.equal(joinProgress(snap(), { bootIdAtSend: "b1", waiting: true }).phase, "searching");
  });

  it("response \"waiting for members\", the node is restarting — \"Joining\"", () => {
    assert.equal(joinProgress(snap({}, { status: "starting" }), { bootIdAtSend: "b1", waiting: true }).phase, "entering");
    assert.equal(joinProgress(snap({ boot_id: "b2" }), { bootIdAtSend: "b1", waiting: true }, "silent").phase, "entering");
    assert.equal(joinProgress(snap({ boot_id: "b2" }), { bootIdAtSend: "b1", waiting: true }), null);
  });

  it("waiting comes before \"sent\"; refused before both", () => {
    assert.equal(joinProgress(snap({}, { invite_join: info() }), { bootIdAtSend: "b1" }).phase, "searching");
    assert.equal(joinProgress(snap({}, { invite_join: info({ state: "refused" }) }), { bootIdAtSend: "b1" }).phase, "refused");
  });
});
