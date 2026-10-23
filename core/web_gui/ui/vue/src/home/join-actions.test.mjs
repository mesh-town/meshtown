/* Joining a network and creating a network without Vue (`join-actions.ts`): highlighting the error word,
 * outcomes of `joinNetwork` and `createNetwork`. The promise after joining is in `join-promise.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError, API_ERROR_TEXT } from "../core/api.ts";
import { NET_NAME } from "../common/words.ts";
import {
  JOIN_PROMISE_MS, JOIN_STALLED, JOIN_WAITING, createNetwork, highlightParts, joinNetwork,
} from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const MARK_KEY = "meshtown.homeSetup";
const plain = (v) => JSON.parse(JSON.stringify(v));

describe("highlightParts", () => {
  it("the word is found in the message — three parts, the middle one with mark", () => {
    const parts = highlightParts("The word \"zebra\" is not in the dictionary", "zebra");
    assert.equal(parts.length, 3);
    assert.deepEqual(parts.map((p) => p.highlighted), [false, true, false]);
    assert.equal(parts[1].text, "\"zebra\"");
    assert.equal(parts.map((p) => p.text).join(""), "The word \"zebra\" is not in the dictionary");
  });

  it("the word is not in the message — it is appended at the end with mark", () => {
    const parts = highlightParts("The key doesn't fit", "zebra");
    const last = parts[parts.length - 1];
    assert.deepEqual(last, { text: "zebra", highlighted: true });
    assert.equal(parts.map((p) => p.text).join(""), "The key doesn't fit zebra");
  });

  it("no word — one part without mark", () => {
    assert.deepEqual(highlightParts("The key doesn't fit", undefined), [{ text: "The key doesn't fit", highlighted: false }]);
    assert.equal(highlightParts("The key doesn't fit", "").length, 1);
  });

  it("<b> in the text stays text, not markup", () => {
    const parts = highlightParts("The word \"<b>x</b>\" is bad", "<b>x</b>");
    assert.equal(parts.map((p) => p.text).join(""), "The word \"<b>x</b>\" is bad");
    assert.ok(parts.every((p) => typeof p.text === "string" && typeof p.highlighted === "boolean"));
  });
});

describe("joinNetwork", () => {
  describe("field is empty", () => {
    it("\"empty\", no requests, under the field a request to paste the invite", async () => {
      const w = setup({});
      w.form.inviteInput = "   ";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "empty");
      assert.equal(w.calls.length, 0);
      assert.equal(inviteErrorText(w.form), "Paste the invite a network member sent you.");
    });
  });

  describe("success", () => {
    it("POST {key}, the field is empty, the status names the network", async () => {
      const w = setup({ reply: { label: "family", restart: true } });
      w.form.inviteInput = " mesh-invite2-abc ";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "joined");
      assert.deepEqual(w.calls, [["POST", "/api/network/join", { key: "mesh-invite2-abc" }]]);
      assert.equal(w.form.inviteInput, "");
      assert.match(w.form.status, /network "family"/);
      assert.equal(w.form.status, "Done: Mesh Town is restarting and joining the network \"family\". The status will appear here in a few seconds.");
      assert.equal(w.form.busy, "");
    });

    it("network without a label — the status without \"null\" and without a dash placeholder", async () => {
      const w = setup({ reply: { label: null, restart: true } });
      w.form.inviteInput = "k";
      await joinNetwork(w.form, w.store, w.deps);
      assert.ok(w.form.status.length > 0, "there is a status");
      assert.doesNotMatch(w.form.status, /null|undefined|"—"/);
    });

    it("node is already in a network (switching from \"Settings\") — the setup mark is not written", async () => {
      const w = setup({ reply: { label: "family" }, inNet: true });
      w.form.inviteInput = "k";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "joined");
      assert.equal(w.storage.data.has(MARK_KEY), false);
    });

    it("setup mark — \"browser\"", async () => {
      const w = setup({ reply: { label: "family" } });
      w.form.inviteInput = "k";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(w.storage.data.get(MARK_KEY), "browser");
    });
  });

  describe("invite mesh-invite2- (waiting response)", () => {
    it("\"waiting\", status JOIN_WAITING, field cleared, the key in the request body once", async () => {
      const w = setup({ reply: { label: "family", restart: false, waiting: true } });
      w.form.inviteInput = " mesh-invite2-abc ";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "waiting");
      assert.equal(w.form.status, JOIN_WAITING);
      assert.equal(w.form.inviteInput, "");
      assert.deepEqual(w.calls, [["POST", "/api/network/join", { key: "mesh-invite2-abc" }]]);
    });

    it("the JOIN_PROMISE_MS deadline is not set, no JOIN_STALLED failure", async () => {
      const w = setup({ reply: { label: null, restart: false, waiting: true } });
      w.form.inviteInput = "mesh-invite2-abc";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(w.timers.length, 0);
      assert.ok(!w.timers.some((t) => t.ms === JOIN_PROMISE_MS));
      assert.notEqual(w.form.status, JOIN_STALLED);
      assert.equal(w.form.status, JOIN_WAITING);
    });

    it("response without waiting (mesh1- key) — the usual \"Done\" and the deadline", async () => {
      const w = setup({ reply: { label: "family", restart: true } });
      w.form.inviteInput = "mesh1-abc";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "joined");
      assert.equal(w.timers.length, 1);
    });
  });

  describe("node refused", () => {
    it("422 with a word — the node message and the word the component will highlight", async () => {
      const body = { message: "The word \"zebra\" is not in the dictionary", word: "zebra" };
      const w = setup({ fail: new ApiError(422, "invalid_key", "x", body) });
      w.form.inviteInput = "k";
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "failed");
      assert.deepEqual(plain(w.form.inviteError), { message: "The word \"zebra\" is not in the dictionary", word: "zebra" });
      const marked = highlightParts(w.form.inviteError.message, w.form.inviteError.word).filter((p) => p.highlighted);
      assert.deepEqual(marked.map((p) => p.text), ["\"zebra\""]);
      assert.equal(w.form.inviteInput, "k", "what was typed is not erased");
    });

    it("422 without message — fallback text", async () => {
      const w = setup({ fail: new ApiError(422, "invalid_key", "x", {}) });
      w.form.inviteInput = "k";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(
        inviteErrorText(w.form),
        "This doesn't look like an invite. Copy it from the network member again in full and paste it here."
      );
    });

    it("other error — \"Couldn't join the network: {reason}\"; empty text — \"— try again\"", async () => {
      const w = setup({ fail: new ApiError(500, "http_500", "The node returned error 500") });
      w.form.inviteInput = "k";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(inviteErrorText(w.form), "Couldn't join the network: the node returned error 500");
      const v = setup({ fail: new Error("") });
      v.form.inviteInput = "k";
      await joinNetwork(v.form, v.store, v.deps);
      assert.equal(inviteErrorText(v.form), "Couldn't join the network — try again");
    });

    it("after a refusal the button is free again, the setup mark is not written", async () => {
      const w = setup({ fail: new ApiError(500, "http_500", "failure") });
      w.form.inviteInput = "k";
      await joinNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.busy, "");
      assert.equal(w.storage.data.has(MARK_KEY), false);
    });
  });

  describe("repeat click in flight", () => {
    it("\"skipped\", one request", async () => {
      const w = setup({ hold: true, reply: { label: "a" } });
      w.form.inviteInput = "k";
      const first = joinNetwork(w.form, w.store, w.deps);
      await tick();
      assert.notEqual(w.form.busy, "", "request in progress — the form is busy");
      assert.equal(await joinNetwork(w.form, w.store, w.deps), "skipped");
      w.release();
      assert.equal(await first, "joined");
      assert.equal(w.calls.length, 1);
    });
  });
});

describe("createNetwork", () => {
  describe("name is empty", () => {
    it("\"empty\", no requests, a request to come up with a name", async () => {
      const w = setup({});
      assert.equal(await createNetwork(w.form, w.store, w.deps), "empty");
      assert.equal(w.calls.length, 0);
      assert.equal(w.form.labelError, "Enter a name, for example \"family\".");
    });
  });

  describe("success", () => {
    it("POST {label}, setup mark \"browser\", create block collapsed, fields clean", async () => {
      const w = setup({ reply: { key: "mesh-SECRETKEY-123", label: "family", restart: true } });
      w.form.label = " family ";
      w.form.choice = "create";
      w.form.inviteInput = "half-typed";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
      assert.deepEqual(w.calls, [["POST", "/api/network/create", { label: "family" }]]);
      assert.equal(w.storage.data.get(MARK_KEY), "browser");
      assert.equal(w.form.choice, "");
      assert.equal(w.form.label, "");
      assert.equal(w.form.inviteInput, "");
      assert.equal(w.form.inviteError, null);
    });

    it("the key from the response does not reach form", async () => {
      const w = setup({ reply: { key: "mesh-SECRETKEY-123", label: "family" } });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.doesNotMatch(JSON.stringify(w.form), /SECRETKEY/);
    });

    it("node reports the created network: no notice for \"Invite a member\" is put on Home, form.status is the short sentence", async () => {
      const w = setup({ reply: { key: "k", label: "family" } });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.deepEqual(w.notices, [], "the invite card's status stays empty after creation");
      assert.equal(w.form.status, "Network \"family\" created.");
      assert.doesNotMatch(w.form.status, /SECRET|null|Click|Invite a member/);
    });

    it("node is already in a network — the setup mark is not written", async () => {
      const w = setup({ reply: { key: "k", label: "family" }, inNet: true });
      w.form.label = "family";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
      assert.equal(w.storage.data.has(MARK_KEY), false);
    });

    it("no deps.notice (\"Settings\") — the text stays in form.status and names the network", async () => {
      const w = setup({ reply: { key: "k", label: "family" }, noNotice: true });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.status, "Network \"family\" created.");
    });

    it("storage unavailable — the network is created anyway, no exception", async () => {
      const w = setup({ reply: { key: "k", label: "family" }, brokenStorage: true });
      w.form.label = "family";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "created");
    });
  });

  describe("node refused", () => {
    it("error text under the name field, the setup mark is not written", async () => {
      const w = setup({ fail: new ApiError(422, "invalid", "name is taken") });
      w.form.label = "family";
      assert.equal(await createNetwork(w.form, w.store, w.deps), "failed");
      assert.equal(w.form.labelError, "Name is taken");
      assert.equal(w.storage.data.has(MARK_KEY), false);
      assert.equal(w.form.busy, "");
    });

    it("timeout: the program name under the field is written as is, as on the whole page", async () => {
      const w = setup({ fail: new ApiError(0, "timeout", API_ERROR_TEXT.timeout) });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.labelError, NET_NAME + " didn't respond in time");
    });

    it("empty error text — \"Couldn't create the network — try again\"", async () => {
      const w = setup({ fail: new Error("") });
      w.form.label = "family";
      await createNetwork(w.form, w.store, w.deps);
      assert.equal(w.form.labelError, "Couldn't create the network — try again");
    });
  });

  describe("repeat click in flight", () => {
    it("\"skipped\", one request", async () => {
      const w = setup({ hold: true, reply: { key: "k", label: "a" } });
      w.form.label = "a";
      const first = createNetwork(w.form, w.store, w.deps);
      await tick();
      assert.equal(await createNetwork(w.form, w.store, w.deps), "skipped");
      w.release();
      assert.equal(await first, "created");
      assert.equal(w.calls.length, 1);
    });
  });
});

// `form.inviteError` — `{ message, word }` or null; `form.labelError` — a string or null.
const inviteErrorText = (form) => form.inviteError.message;

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
