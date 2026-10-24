/* Creating a network of a chosen kind through the add form (`add-net-actions.ts`): the kind rides in the create request only
 * when it is "Managed", and the banner after creation depends on it. The node is a fake `api` that records every call. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { EV_CHANGED } from "../core/bridge.ts";
import { createStore } from "../core/store.ts";
import { addNetwork, addedBannerText } from "./add-net-actions.ts";
import { createdBanner } from "./add-net-texts.ts";
import { createJoinForm } from "./join-actions.ts";

const NEW_SLOT = "k7q2xa";
const KEY = "mesh-invite2-alpha-beta-gamma";
const CREATE_PATH = `/api/net/${NEW_SLOT}/network/create`;
const JOIN_PATH = `/api/net/${NEW_SLOT}/network/join`;

const PEER_BANNER = 'You created "family". Nobody else is here yet: use Invite to "family" to add a member.';
const MANAGED_BANNER =
  'You created "family" and you\'re its admin. Nobody else is here yet: use Invite to "family" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";

describe("the form", () => {
  it("starts as a Peer network", () => {
    assert.equal(createJoinForm().kind, "Peer");
  });
});

describe("create", () => {
  describe("Peer, the default", () => {
    it("sends the name and nothing else", async () => {
      const w = setup({ label: "family" });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "created");
      assert.deepEqual(w.bodyOf(CREATE_PATH), { label: "family" });
    });

    it("an explicit Peer is sent the same way, without a kind", async () => {
      const w = setup({ label: "family", kind: "Peer" });
      await addNetwork(w.form, w.store, w.deps, "create");
      assert.deepEqual(w.bodyOf(CREATE_PATH), { label: "family" });
    });
  });

  describe("Managed", () => {
    it("sends the trimmed name and the kind", async () => {
      const w = setup({ label: "  family ", kind: "Managed" });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "created");
      assert.deepEqual(w.bodyOf(CREATE_PATH), { label: "family", kind: "Managed" });
    });

    it("takes a slot first, like any other create", async () => {
      const w = setup({ label: "family", kind: "Managed" });
      await addNetwork(w.form, w.store, w.deps, "create");
      assert.deepEqual(w.calls.map((c) => c[1]), ["/api/nets", CREATE_PATH]);
    });

    it("an empty name is refused locally, nothing is sent", async () => {
      const w = setup({ label: "   ", kind: "Managed" });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "empty");
      assert.deepEqual(w.calls, []);
    });

    it("at the limit nothing is sent either", async () => {
      const w = setup({ label: "family", kind: "Managed", nets: 8 });
      assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "limit");
      assert.deepEqual(w.calls, []);
    });
  });

  it("a refused create reports a failure and keeps the name for another try", async () => {
    const w = setup({ label: "family", kind: "Managed", routes: { [`POST ${CREATE_PATH}`]: new Error("boom") } });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "failed");
    assert.equal(w.form.label, "family");
    assert.notEqual(w.form.labelError, null);
  });
});

describe("join", () => {
  it("never carries a kind: the key alone is sent, whatever the form holds", async () => {
    const w = setup({ input: KEY, kind: "Managed" });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "joined");
    assert.deepEqual(w.bodyOf(JOIN_PATH), { key: KEY });
  });
});

describe("the banner after creating", () => {
  it("a Managed network says you are its admin and asks for a second admin", async () => {
    const w = setup({ label: "family", kind: "Managed" });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.equal(addedBannerText(w.store, w.form, result), MANAGED_BANNER);
  });

  it("a Peer network keeps the former text", async () => {
    const w = setup({ label: "family", kind: "Peer" });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.equal(addedBannerText(w.store, w.form, result), PEER_BANNER);
  });

  it("with several networks the text is the same: a created banner never mentions an exit", async () => {
    const w = setup({ label: "family", kind: "Managed", nets: 3 });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.equal(addedBannerText(w.store, w.form, result), MANAGED_BANNER);
  });

  describe("createdBanner", () => {
    it("Managed", () => assert.equal(createdBanner("family", "Managed"), MANAGED_BANNER));
    it("Peer", () => assert.equal(createdBanner("family", "Peer"), PEER_BANNER));
    it("without a kind — Peer", () => assert.equal(createdBanner("family"), PEER_BANNER));
  });
});

// Preconditions: what is typed in the form (`kind` too), how many networks are shown, how the node answers each "METHOD path"
// (an `Error` is thrown, anything else is the reply). Unlisted requests get `{}`, `POST /api/nets` a fresh slot, and the create
// route answers with the typed name.
function setup({ input = "", label = "", kind = undefined, routes = {}, nets = 1 } = {}) {
  const store = createStore();
  const ids = ["main", ...Array.from({ length: nets - 1 }, (_, i) => "aaaaa" + (i + 2))];
  store.nets = ids.map((id) => ({ id, label: id, id_hex: "hex-" + id, joining: false }));
  store.slots = Object.fromEntries(ids.map((id) => [id, { state: { network: { id_hex: "hex-" + id } } }]));
  const form = createJoinForm();
  form.inviteInput = input;
  form.label = label;
  if (kind) form.kind = kind;
  const calls = [];
  const api = async (method, path, body) => {
    calls.push([method, path, structuredClone(body)]);
    const answer = routes[`${method} ${path}`] ?? answerOf(method, path, label);
    if (answer instanceof Error) throw answer;
    return answer;
  };
  const bus = new EventTarget();
  bus.addEventListener(EV_CHANGED, () => {});
  vi.stubGlobal("document", bus);
  const deps = { api, storage: null, setTimer: () => 0, clearTimer: () => {}, notice: () => {} };
  const bodyOf = (path) => calls.find((c) => c[1] === path)?.[2];
  return { form, store, deps, calls, bodyOf };
}

function answerOf(method, path, label) {
  if (path === "/api/nets") return { slot: NEW_SLOT, fresh: true, nets: [] };
  if (path === CREATE_PATH) return { key: "mesh1-secret", label: label.trim(), restart: false };
  if (path === JOIN_PATH) return { label: "friends", restart: false };
  return {};
}
