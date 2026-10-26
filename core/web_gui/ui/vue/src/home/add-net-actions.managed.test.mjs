/* Creating a network through the add form (`add-net-actions.ts`): the create request carries the name only, and the banner
 * after creation tells the creator to name a second admin. The node is a fake `api` that records every call. */

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

const BANNER =
  'You created "family" and you\'re its admin. Nobody else is here yet: use Invite to "family" to add a member. ' +
  "Once someone joins, make a second member an admin (⋯ → Make admin): a network without an admin can't be managed.";

describe("create", () => {
  it("sends the trimmed name and nothing else", async () => {
    const w = setup({ label: "  family " });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "created");
    assert.deepEqual(w.bodyOf(CREATE_PATH), { label: "family" });
  });

  it("takes a slot first, like any other create", async () => {
    const w = setup({ label: "family" });
    await addNetwork(w.form, w.store, w.deps, "create");
    assert.deepEqual(w.calls.map((c) => c[1]), ["/api/nets", CREATE_PATH]);
  });

  it("an empty name is refused locally, nothing is sent", async () => {
    const w = setup({ label: "   " });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "empty");
    assert.deepEqual(w.calls, []);
  });

  it("at the limit nothing is sent either", async () => {
    const w = setup({ label: "family", nets: 8 });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "limit");
    assert.deepEqual(w.calls, []);
  });

  it("a refused create reports a failure and keeps the name for another try", async () => {
    const w = setup({ label: "family", routes: { [`POST ${CREATE_PATH}`]: new Error("boom") } });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "create"), "failed");
    assert.equal(w.form.label, "family");
    assert.notEqual(w.form.labelError, null);
  });
});

describe("join", () => {
  it("the key alone is sent", async () => {
    const w = setup({ input: KEY });
    assert.equal(await addNetwork(w.form, w.store, w.deps, "join"), "joined");
    assert.deepEqual(w.bodyOf(JOIN_PATH), { key: KEY });
  });
});

describe("the banner after creating", () => {
  it("says you are the admin and asks for a second admin", async () => {
    const w = setup({ label: "family" });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.equal(addedBannerText(w.store, w.form, result), BANNER);
  });

  it("with several networks the text is the same: a created banner never mentions an exit", async () => {
    const w = setup({ label: "family", nets: 3 });
    const result = await addNetwork(w.form, w.store, w.deps, "create");
    assert.equal(addedBannerText(w.store, w.form, result), BANNER);
  });

  it("createdBanner is the same text", () => assert.equal(createdBanner("family"), BANNER));
});

// Preconditions: what is typed in the form, how many networks are shown, how the node answers each "METHOD path"
// (an `Error` is thrown, anything else is the reply). Unlisted requests get `{}`, `POST /api/nets` a fresh slot, and the create
// route answers with the typed name.
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
  if (path === CREATE_PATH) return { key: "mesh2-secret", label: label.trim(), restart: false };
  if (path === JOIN_PATH) return { label: "friends", restart: false };
  return {};
}
