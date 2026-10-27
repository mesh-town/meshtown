/* `JoiningCard`: the card of a network slot that waits for its members to let this computer in. It says that the join takes up to a
 * minute, after 60 s that nobody answered, and offers "Cancel", which gives the slot back with one `members/leave`. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../core/api.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { shown, visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { settle } from "../testing/page.mjs";
import JoiningCard from "./JoiningCard.vue";

const WAITING = "neta22";
const LEAVE_PATH = "/api/net/" + WAITING + "/members/leave";
const JOINING = "Joining the network…";
const MINUTE = "This takes up to a minute. Your other networks keep working.";
const NO_ANSWER = "No answer yet — the network's host may be offline.";
const CANCEL_FAILED = "Couldn't cancel — try again.";
const NO_ANSWER_MS = 60_000;

let mounted;

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  const unexpected = mounted?.node.unexpected ?? [];
  mounted?.wrapper.unmount();
  mounted = null;
  vi.useRealTimers();
  expect(unexpected).toEqual([]);
});

describe("just after the join started", () => {
  it("says that the network is being joined and that it takes up to a minute", async () => {
    const card = await setup();
    expect(card.text()).toContain(JOINING);
    expect(card.text()).toContain(MINUTE);
  });

  it("does not claim yet that nobody answered", async () => {
    const card = await setup();
    expect(card.text()).not.toContain(NO_ANSWER);
  });

  it("is a status region with the Cancel button and no error", async () => {
    const card = await setup();
    expect(card.wrapper.get("#net-joining").attributes("role")).toBe("status");
    expect(card.cancelText()).toBe("Cancel");
    expect(card.error()).toBe("");
  });

  it("sends nothing until Cancel is pressed", async () => {
    const card = await setup();
    expect(card.postedPaths()).toEqual([]);
  });
});

describe("the members refused the join", () => {
  const MESSAGE = "This invite can't be used: whoever made it is not a network admin now. Ask a network admin for a new one.";

  it("shows the node message verbatim instead of the waiting lines", async () => {
    const card = await setup(MESSAGE);
    expect(compact(card.wrapper.get("#net-join-refused").text())).toBe(MESSAGE);
    expect(card.text()).not.toContain(JOINING);
    expect(card.text()).not.toContain(MINUTE);
  });

  it("the button says Back and gives the slot back", async () => {
    const card = await setup(MESSAGE);
    expect(card.cancelText()).toBe("Back");
    await card.when.cancelled();
    expect(card.postedPaths()).toEqual([LEAVE_PATH]);
  });
});

describe("the minute passes", () => {
  it("one millisecond short of 60 s it still says up to a minute", async () => {
    const card = await setup();
    await vi.advanceTimersByTimeAsync(NO_ANSWER_MS - 1);
    expect(card.text()).toContain(MINUTE);
    expect(card.text()).not.toContain(NO_ANSWER);
  });

  it("at 60 s the hint is replaced by the no-answer text", async () => {
    const card = await setup();
    await vi.advanceTimersByTimeAsync(NO_ANSWER_MS);
    await settle();
    expect(card.text()).toContain(NO_ANSWER);
    expect(card.text(), "\"up to a minute\" is gone").not.toContain(MINUTE);
  });

  it("the joining line stays and Cancel is still there", async () => {
    const card = await setup();
    await vi.advanceTimersByTimeAsync(NO_ANSWER_MS);
    await settle();
    expect(card.text()).toContain(JOINING);
    expect(card.cancelText()).toBe("Cancel");
  });

  it("the timer is cleared when the card goes away", async () => {
    const timersBefore = vi.getTimerCount();
    const card = await setup();
    expect(vi.getTimerCount(), "the card waits for the minute").toBeGreaterThan(timersBefore);
    card.wrapper.unmount();
    mounted = null;
    expect(vi.getTimerCount()).toBe(timersBefore);
  });
});

describe("Cancel", () => {
  describe("the node accepts it", () => {
    it("leaves the slot once: one members/leave with confirm", async () => {
      const card = await setup();
      await card.when.cancelled();
      expect(card.postedPaths()).toEqual([LEAVE_PATH]);
      expect(card.postedBodies()).toEqual([{ confirm: true }]);
    });

    it("tells the parent with \"cancelled\"", async () => {
      const card = await setup();
      await card.when.cancelled();
      expect(card.wrapper.emitted("cancelled")).toHaveLength(1);
    });

    it("drops the slot from the page", async () => {
      const card = await setup();
      await card.when.cancelled();
      expect(card.store.nets.map((n) => n.id)).not.toContain(WAITING);
      expect(card.store.slots[WAITING]).toBeUndefined();
    });

    it("works after the no-answer text has appeared", async () => {
      const card = await setup();
      await vi.advanceTimersByTimeAsync(NO_ANSWER_MS);
      await settle();
      await card.when.cancelled();
      expect(card.postedPaths()).toEqual([LEAVE_PATH]);
      expect(card.wrapper.emitted("cancelled")).toHaveLength(1);
    });
  });

  describe("while the request is on its way", () => {
    it("the button is disabled, so a second press sends nothing", async () => {
      const card = await setup();
      const gate = card.holdReplies();
      await card.when.pressedCancel();
      expect(card.wrapper.get("#net-join-cancel").attributes("disabled")).toBeDefined();
      await card.when.pressedCancel();
      expect(card.postedPaths()).toEqual([LEAVE_PATH]);
      gate.release();
      await settle();
    });
  });

  describe("the node refuses or does not answer", () => {
    it("shows the error next to the card, which stays where it is", async () => {
      const card = await setup();
      card.failsReplies();
      await card.when.cancelled();
      expect(card.error()).toBe(CANCEL_FAILED);
      expect(card.wrapper.find("#net-joining").exists()).toBe(true);
      expect(card.text()).toContain(JOINING);
    });

    it("tells the parent nothing and keeps the slot", async () => {
      const card = await setup();
      card.failsReplies();
      await card.when.cancelled();
      expect(card.wrapper.emitted("cancelled")).toBeUndefined();
      expect(card.store.nets.map((n) => n.id)).toContain(WAITING);
    });

    it("the button is usable again", async () => {
      const card = await setup();
      card.failsReplies();
      await card.when.cancelled();
      expect(card.wrapper.get("#net-join-cancel").attributes("disabled")).toBeUndefined();
    });
  });
});

async function setup(refused) {
  const opts = twoNets();
  opts.slots.push({ id: WAITING });
  const node = createFakeNode(opts);
  node.slotList.find((s) => s.id === WAITING).joining = true;
  vi.stubGlobal("fetch", node.fetch);
  sessionStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  acceptOverview(store, parseOverview(await api("GET", "/api/overview")));
  const wrapper = mount(JoiningCard, { props: { slot: WAITING, refused }, attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  await settle();
  mounted = { wrapper, node };
  return cardOf(wrapper, node, store);
}

const compact = (text) => text.replace(/\s+/g, " ").trim();

function cardOf(wrapper, node, store) {
  const root = () => wrapper.get("#net-joining");
  const posts = () => node.calls.filter((c) => c.method === "POST");
  const pressedCancel = async () => {
    await wrapper.get("#net-join-cancel").trigger("click");
    await settle();
  };
  return {
    wrapper, node, store,
    text: () => compact(visibleText(root().element)),
    cancelText: () => compact(wrapper.get("#net-join-cancel").text()),
    error() {
      const el = wrapper.find("#net-join-error");
      return el.exists() && shown(el.element) ? compact(el.text()) : "";
    },
    postedPaths: () => posts().map((c) => c.path),
    postedBodies: () => posts().map((c) => c.body),
    when: {
      pressedCancel,
      async cancelled() {
        await pressedCancel();
        await settle();
      },
    },
    // The node answers every request with a failure.
    failsReplies() {
      vi.stubGlobal("fetch", async () => new Response(JSON.stringify({ error: "internal" }), { status: 500, headers: { "Content-Type": "application/json" } }));
    },
    // The node's replies wait for `release()`; requests are still recorded by the fake node.
    holdReplies() {
      let release = () => {};
      const gate = new Promise((resolve) => { release = resolve; });
      vi.stubGlobal("fetch", async (input, init) => {
        const pending = node.fetch(input, init);
        await gate;
        return pending;
      });
      return { release: () => release() };
    },
  };
}
