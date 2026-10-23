/* The peers tab of the shell in the address hash (`store.peersTab`). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_B } from "../testing/overview-fixtures.mjs";
import { STATE, build, settle } from "./use-shell-support.mjs";

describe("the peers tab in the address hash (store.peersTab)", () => {
  describe("the overview has the networks A (main) and B", () => {
    it("#peers/<B> — tab B, section Peers", async () => {
      const w = await mountPeers({ hash: "#peers/" + NET_B });
      assert.equal(w.store.peersTab, NET_B);
      assert.equal(w.store.activeSection, "peers");
    });

    it("#peers — the first visible network, A", async () => {
      const w = await mountPeers({ hash: "#peers" });
      assert.equal(w.store.peersTab, "main");
    });

    it("#peers/add — the add form", async () => {
      const w = await mountPeers({ hash: "#peers/add" });
      assert.equal(w.store.peersTab, "add");
    });

    it("#peers/zzzzzz, a network that does not exist — the first visible network, A", async () => {
      const w = await mountPeers({ hash: "#peers/zzzzzz" });
      assert.equal(w.store.peersTab, "main");
      assert.equal(w.store.activeSection, "peers");
    });

    it("hashchange to #peers/<B> — tab B", async () => {
      const w = await mountPeers({ hash: "#peers" });
      w.goTo("#peers/" + NET_B);
      await settle();
      assert.equal(w.store.peersTab, NET_B);
    });

    it("hashchange from #peers/<B> to #peers/main — tab A", async () => {
      const w = await mountPeers({ hash: "#peers/" + NET_B });
      w.goTo("#peers/main");
      await settle();
      assert.equal(w.store.peersTab, "main");
    });
  });

  describe("the main network has no key, so only B is visible", () => {
    it("#peers/main — the first visible network, B", async () => {
      const w = await mountPeers({ hash: "#peers/main", keyless: ["main"] });
      assert.equal(w.store.peersTab, NET_B);
    });
  });

  describe("the tab is B and B disappears from the next overview", () => {
    it("the tab falls back to A and the address hash is not rewritten", async () => {
      // given
      const w = await mountPeers({ hash: "#peers/" + NET_B });
      assert.equal(w.store.peersTab, NET_B, "precondition: the tab is B");
      // when
      w.nets = ["main"];
      await w.pollAgain();
      // then
      assert.equal(w.store.peersTab, "main");
      assert.equal(w.deps.hash, "#peers/" + NET_B, "the hash stays, so \"Back\" follows the browser history");
      assert.deepEqual(w.replacedUrls, [], "no address replacement");
    });
  });

  describe("no overview has arrived yet", () => {
    it("#peers/<B> — the tab is B, without checking that B exists", async () => {
      const w = await mountPeers({ hash: "#peers/" + NET_B, neverAnswered: true });
      assert.equal(w.store.source, null, "precondition: no overview yet");
      assert.equal(w.store.peersTab, NET_B);
    });
  });

  describe("the page is opened with ?token=… and #peers/<B>", () => {
    it("the token is taken off the address, the hash and tab B are kept", async () => {
      // given / when
      const w = await mountPeers({ hash: "#peers/" + NET_B, href: "http://h/?token=abc#peers/" + NET_B });
      // then
      assert.equal(w.storage.getItem("meshtown.token"), "abc");
      assert.deepEqual(w.replacedUrls, ["/#peers/" + NET_B], "the cleaned address has no token but keeps the hash");
      assert.equal(w.store.activeSection, "peers");
      assert.equal(w.store.peersTab, NET_B);
    });
  });
});

// `GET /api/overview` of the networks `ids`; a network in `keyless` has no `id_hex`, so it is hidden.
function overviewOf(ids, keyless) {
  const brief = (id) => ({
    id, label: id === "main" ? "friends" : "my devices", id_hex: keyless.includes(id) ? null : "net-" + id,
    status: "online", exit_enabled: false, proxy: id === "main", route_state: "direct", exits: [],
    members_voting: 1, joining: false, members_admitting: 0, me_excluded: false, me_pending: false,
    peers_online: 0, peers_total: 0,
  });
  const entry = (id) => ({ slot: id, state: { ...STATE, boot_id: "boot-" + id }, members: null });
  return { api: 2, nets: ids.map(brief), slots: ids.map(entry), restart_slots: [] };
}

// A shell fed by the overview of A (`main`) and B; `neverAnswered` — the overview request hangs.
async function mountPeers({ hash, href, keyless = [], neverAnswered = false }) {
  const overview = (e) => (neverAnswered ? new Promise(() => {}) : overviewOf(e.nets, keyless));
  const env = build({ hash, href, overview });
  env.goTo = (next) => { env.deps.hash = next; env.win.fire("hashchange"); };
  env.pollAgain = async () => { await env.fireAllTimers(); await settle(); };
  await settle();
  return env;
}
