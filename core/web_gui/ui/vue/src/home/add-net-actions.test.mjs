/* Adding a network next to the existing ones (`add-net-actions.ts`): `POST /api/nets` takes an empty slot, then the invite or the
 * name goes to that slot's own address. The node is a fake `api` that answers by "METHOD path"; the outcome is the result of
 * `addNetwork` plus what is left in the form. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { EV_CHANGED } from "../core/bridge.ts";
import { createStore } from "../core/store.ts";
import { addNetwork, addedBannerText } from "./add-net-actions.ts";
import { createJoinForm } from "./join-actions.ts";
import { HOME_INVITE_EMPTY, HOME_LABEL_EMPTY } from "./home-flow-texts.ts";

const NEW_SLOT = "k7q2xa";
const OTHER_SLOT = "b3d5fg";
const KEY = "mesh-invite2-alpha-beta-gamma";
const NETS_REPLY = { slot: NEW_SLOT, nets: [] };

describe("addNetwork", () => {
  describe("eight networks are already shown", () => {
    it("join — limit, nothing is sent", async () => {
      const w = setup({ input: KEY, nets: 8 });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "limit");
      assert.deepEqual(w.calls, []);
    });

    it("create — limit, nothing is sent", async () => {
      const w = setup({ label: "family", nets: 8 });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "limit");
      assert.deepEqual(w.calls, []);
    });

    it("seven networks — still allowed", async () => {
      const w = setup({ input: KEY, nets: 7 });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "joined");
    });
  });

  describe("a request is in flight", () => {
    it("a second press is skipped, no extra requests", async () => {
      const w = setup({ input: KEY });
      w.form.busy = "join";
      assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "skipped");
      assert.deepEqual(w.calls, []);
    });
  });

  describe("join", () => {
    describe("invite typed", () => {
      it("takes a slot first, then sends the key to that slot's join address", async () => {
        const w = setup({ input: KEY });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.deepEqual(w.calls, [
          ["POST", "/api/nets", {}],
          ["POST", `/api/net/${NEW_SLOT}/network/join`, { key: KEY }],
        ]);
      });

      it("spaces around the invite are not sent", async () => {
        const w = setup({ input: `  ${KEY}\n` });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.deepEqual(w.calls[1][2], { key: KEY });
      });

      it("the node accepted it at once — joined", async () => {
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: { label: "friends" } } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "joined");
      });

      it("the invite is gone from the form and the form is not busy", async () => {
        const w = setup({ input: KEY });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.equal(w.form.inviteInput, "");
        assert.equal(w.form.busy, "");
        assert.equal(w.form.inviteError, null);
      });

      it("the node answers waiting — waiting", async () => {
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: { waiting: true } } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "waiting");
        assert.equal(w.form.inviteInput, "", "the key does not stay in the form while waiting");
      });
    });

    describe("hidden main slot", () => {
      it("the node hands back main — the key goes to the unprefixed join address", async () => {
        const w = setup({ input: KEY, routes: { "POST /api/nets": { slot: "main", nets: [] } } });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.deepEqual(w.calls.map((c) => c.slice(0, 2)), [["POST", "/api/nets"], ["POST", "/api/network/join"]]);
      });
    });

    describe("nothing typed", () => {
      it("empty — no requests at all", async () => {
        const w = setup({ input: "   " });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "empty");
        assert.deepEqual(w.calls, []);
      });

      it("the field explains what to paste", async () => {
        const w = setup({ input: "" });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.deepEqual(w.form.inviteError, { message: HOME_INVITE_EMPTY, word: null });
      });
    });

    describe("network already added", () => {
      it("409 net_already_added — duplicate, with the slot that holds it", async () => {
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: alreadyAdded(OTHER_SLOT) } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "duplicate");
        assert.equal(w.form.dupSlot, OTHER_SLOT);
      });

      it("the empty slot is not removed — no DELETE", async () => {
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: alreadyAdded(OTHER_SLOT) } });
        await addNetwork(w.form, w.store, w.deps, "join");
        assert.ok(w.calls.every(([method]) => method === "POST"), "only POSTs were sent");
      });
    });

    describe("too many networks", () => {
      it("409 too_many_nets on taking the slot — limit, the key is not sent", async () => {
        const w = setup({ input: KEY, routes: { "POST /api/nets": new ApiError(409, "too_many_nets", "too many networks", { limit: 8 }) } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "limit");
        assert.deepEqual(w.calls.map((c) => c[1]), ["/api/nets"]);
      });
    });

    describe("bad invite", () => {
      it("422 invalid_key — failed, the word is highlighted in the field error", async () => {
        const err = new ApiError(422, "invalid_key", "bad key", { message: "The word \"zebra\" is not in the dictionary", word: "zebra" });
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: err } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "failed");
        assert.deepEqual(w.form.inviteError, { message: "The word \"zebra\" is not in the dictionary", word: "zebra" });
      });

      it("another refusal — failed, the field carries a \"Couldn't …\" text without a highlighted word", async () => {
        const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: new ApiError(500, "internal", "boom") } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "failed");
        assert.match(w.form.inviteError.message, /^Couldn't /);
        assert.equal(w.form.inviteError.word, null);
        assert.equal(w.form.busy, "");
      });
    });
  });

  describe("create", () => {
    describe("name typed", () => {
      it("takes a slot, then sends the name to that slot's create address", async () => {
        const w = setup({ label: "family" });
        await addNetwork(w.form, w.store, w.deps, "create");
        assert.deepEqual(w.calls, [
          ["POST", "/api/nets", {}],
          ["POST", `/api/net/${NEW_SLOT}/network/create`, { label: "family" }],
        ]);
      });

      it("created", async () => {
        const w = setup({ label: "family", routes: { [`POST /api/net/${NEW_SLOT}/network/create`]: { label: "family" } } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "created");
        assert.equal(w.form.labelError, null);
      });

      it("the node hands back main — the name goes to the unprefixed create address", async () => {
        const w = setup({ label: "family", routes: { "POST /api/nets": { slot: "main", nets: [] } } });
        await addNetwork(w.form, w.store, w.deps, "create");
        assert.equal(w.calls[1][1], "/api/network/create");
      });
    });

    describe("name empty", () => {
      it("empty — no requests, the field asks for a name", async () => {
        const w = setup({ label: "  " });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "empty");
        assert.deepEqual(w.calls, []);
        assert.equal(w.form.labelError, HOME_LABEL_EMPTY);
      });
    });

    describe("too many networks", () => {
      it("409 too_many_nets — limit, the name is not sent", async () => {
        const w = setup({ label: "family", routes: { "POST /api/nets": new ApiError(409, "too_many_nets", "too many", { limit: 8 }) } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "limit");
        assert.deepEqual(w.calls.map((c) => c[1]), ["/api/nets"]);
      });
    });

    describe("refused", () => {
      it("the node refuses to create — failed, the reason is under the name field", async () => {
        const w = setup({ label: "family", routes: { [`POST /api/net/${NEW_SLOT}/network/create`]: new ApiError(409, "busy", "the node is busy") } });
        assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "failed");
        assert.ok(w.form.labelError, "the name field shows a reason");
        assert.equal(w.form.busy, "");
      });
    });
  });

  describe("after an answer", () => {
    it("joined — the page is asked for an extra poll", async () => {
      const w = setup({ input: KEY });
      await addNetwork(w.form, w.store, w.deps, "join");
      assert.deepEqual(w.changed, ["state"]);
    });

    it("duplicate — the page is asked for an extra poll", async () => {
      const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: alreadyAdded(OTHER_SLOT) } });
      await addNetwork(w.form, w.store, w.deps, "join");
      assert.deepEqual(w.changed, ["state"]);
    });

    it("empty input — no extra poll", async () => {
      const w = setup({ input: "" });
      await addNetwork(w.form, w.store, w.deps, "join");
      assert.deepEqual(w.changed, []);
    });
  });
});

describe("addedBannerText", () => {
  it("joined — the banner names the network from the node's reply", async () => {
    const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: { label: "friends" } } });
    const result = await addNetwork(w.form, w.store, w.deps, "join");
    assert.equal(addedBannerText(w.store, w.form, result), "You joined \"friends\".");
  });

  it("joined — the name comes from the slot's entry, the same as on its tab", async () => {
    const w = setup({ input: KEY, routes: { [`POST /api/net/${NEW_SLOT}/network/join`]: {} } });
    w.store.nets.push({ id: NEW_SLOT, label: undefined, id_hex: "ab12cdef", joining: false });
    const result = await addNetwork(w.form, w.store, w.deps, "join");
    assert.equal(addedBannerText(w.store, w.form, result), "You joined \"Network ab12\".");
  });

  it("created — the banner names the typed network and points to the invite", async () => {
    const w = setup({ label: "family" });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.match(addedBannerText(w.store, w.form, result), /^You created "family"\./);
  });

  it("waiting, duplicate, limit, failed, empty — no banner", async () => {
    const w = setup({ input: KEY });
    for (const result of ["waiting", "duplicate", "limit", "failed", "empty"]) {
      assert.equal(addedBannerText(w.store, w.form, result), null, result);
    }
  });
});

function alreadyAdded(slot) {
  return new ApiError(409, "net_already_added", "already added", { slot });
}

// Preconditions: what is typed in the form, how many networks are shown, and how the node answers each "METHOD path" (an `Error` is thrown, anything else is
// the reply); unlisted requests are answered with `{}`, `POST /api/nets` with a fresh slot. `changed` collects the poll requests.
function setup({ input = "", label = "", routes = {}, nets = 1 } = {}) {
  const store = createStore();
  const ids = ["main", ...Array.from({ length: nets - 1 }, (_, i) => "aaaaa" + (i + 2))];
  store.nets = ids.map((id) => ({ id, label: id, id_hex: "hex-" + id, joining: false }));
  store.slots = Object.fromEntries(ids.map((id) => [id, { state: { network: { id_hex: "hex-" + id } } }]));
  const form = createJoinForm();
  form.inviteInput = input;
  form.label = label;
  const calls = [];
  const api = async (method, path, body) => {
    calls.push([method, path, structuredClone(body)]);
    const answer = routes[`${method} ${path}`] ?? (path === "/api/nets" ? NETS_REPLY : {});
    if (answer instanceof Error) throw answer;
    return answer;
  };
  const changed = [];
  const bus = new EventTarget();
  bus.addEventListener(EV_CHANGED, (e) => changed.push(e.detail.what));
  vi.stubGlobal("document", bus);
  const deps = { api, storage: null, setTimer: () => 0, clearTimer: () => {}, notice: () => {} };
  return { form, store, deps, calls, changed };
}
