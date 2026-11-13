/* `statePanel` for a chosen exit that is overloaded. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { statePanel } from "./home-view.ts";
import { baseState, PEER_FULL } from "../testing/section.mjs";

const exitOf = (over) => ({ id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", country: "DE", exit: true, online: true, active: true, ...over });
const stateWith = (exit, route = "exit") => baseState({ route: { state: route, exits: [exit] } });
const REASON = "It has reached its connection limit.";
const AWAY = "peer-one turned away 3 of your last 5 connections.";
const TAIL = "Some sites and apps may not load.";

describe("overloaded exit panel", () => {
  it("state panel is orange with record and local texts", () => {
    const both = statePanel(stateWith(exitOf({ busy: true, overloaded: "limit", turned_away: { refused: 3, attempts: 5 } })));
    assert.equal(both.level, "orange");
    assert.equal(both.main, "Sites open via peer-one, but it is overloaded");
    assert.equal(both.sub, `${AWAY} ${REASON} ${TAIL}`);
    assert.deepEqual(both.actions, ["pick-other"]);
    const record = statePanel(stateWith(exitOf({ busy: true, overloaded: "limit", turned_away: null })));
    assert.equal(record.sub, `${REASON} ${TAIL}`);
    const local = statePanel(stateWith(exitOf({ busy: true, overloaded: null, turned_away: { refused: 3, attempts: 5 } })));
    assert.equal(local.sub, `${AWAY} ${TAIL}`);
    assert.equal(local.hint, "Choose another member to open sites through, or ask the owner of peer-one to raise its connection limit.");
  });

  it("memory reason changes the advice", () => {
    const p = statePanel(stateWith(exitOf({ busy: true, overloaded: "memory" })));
    assert.equal(p.hint, "Choose another member to open sites through, or ask the owner of peer-one to free up memory on it.");
    assert.equal(p.sub, `It is low on memory and refuses new connections. ${TAIL}`);
  });

  it("red beats orange when the exit is offline", () => {
    const p = statePanel(stateWith(exitOf({ busy: true, overloaded: "limit", online: false }), "unavailable"));
    assert.equal(p.level, "red");
    const online = statePanel(stateWith(exitOf({ busy: true, overloaded: "limit", online: false })));
    assert.equal(online.level, "green");
  });
});
