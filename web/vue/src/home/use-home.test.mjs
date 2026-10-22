/* Observers of `useHome`: the name draft and the exit choice are not overwritten by polling, the notice about
 * creating a network survives the network appearing. A fake store, `withSetup` instead of a component. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { nextTick } from "vue";
import { withSetup } from "../testing/with-setup.mjs";
import { createStore } from "../core/store.ts";
import { pacBlock } from "./pac-setup.ts";
import { useHome } from "./use-home.ts";

describe("useHome > node name", () => {
  describe("field untouched and without focus", () => {
    it("follows the new value from the snapshot", async () => {
      const w = setup();
      w.store.config = withConfig({ name: "new-name" });
      await nextTick();
      assert.equal(w.h.form.name.draft, "new-name");
      w.unmount();
    });
  });

  describe("focus in the field", () => {
    it("the draft is intact when the store changes", async () => {
      const w = setup();
      w.h.form.name.focused = true;
      w.h.form.name.draft = "abc";
      w.store.config = withConfig({ name: "new-name" });
      await nextTick();
      assert.equal(w.h.form.name.draft, "abc");
      w.unmount();
    });

    it("focus is there, the text was not edited — also not overwritten (the cursor does not jump)", async () => {
      const w = setup();
      w.h.form.name.focused = true;
      w.store.config = withConfig({ name: "new-name" });
      await nextTick();
      assert.equal(w.h.form.name.draft, "old-name");
      w.unmount();
    });
  });

  describe("text edited, no focus", () => {
    it("the draft is intact", async () => {
      const w = setup();
      w.h.form.name.draft = "abc";
      w.store.config = withConfig({ name: "new-name" });
      await nextTick();
      assert.equal(w.h.form.name.draft, "abc");
      w.unmount();
    });
  });
});

describe("useHome > exit choice", () => {
  describe("our save is in progress", () => {
    it("a store change does not overwrite exitChoice", async () => {
      const w = setup();
      w.h.form.exitSavesInFlight = 1;
      w.h.form.exitChoice = "P";
      w.store.config = withConfig({ exits: ["Q"] });
      await nextTick();
      assert.equal(w.h.form.exitChoice, "P");
      w.unmount();
    });
  });

  describe("no save", () => {
    it("the choice follows the snapshot", async () => {
      const w = setup();
      w.store.config = withConfig({ exits: ["Q"] });
      await nextTick();
      assert.equal(w.h.form.exitChoice, "Q");
      w.unmount();
    });
  });
});

describe("useHome > network creation notice", () => {
  it("survives the network appearing (null → id_hex)", async () => {
    const w = setup({ network: null });
    w.h.notice.value = "Network \"family\" created.";
    w.store.state = withState({ network: { label: "family", id_hex: "n1" } });
    await nextTick();
    assert.equal(w.h.notice.value, "Network \"family\" created.");
    w.unmount();
  });

  it("goes away when id_hex changes to another", async () => {
    const w = setup({ network: { label: "family", id_hex: "n1" } });
    w.h.notice.value = "Network \"family\" created.";
    w.store.state = withState({ network: { label: "other", id_hex: "n2" } });
    await nextTick();
    assert.equal(w.h.notice.value, "");
    w.unmount();
  });

  it("goes away when the network vanishes", async () => {
    const w = setup({ network: { label: "family", id_hex: "n1" } });
    w.h.notice.value = "Network \"family\" created.";
    w.store.state = withState({ network: null });
    await nextTick();
    assert.equal(w.h.notice.value, "");
    w.unmount();
  });

  it("a poll with the same id_hex does not touch the notice", async () => {
    const w = setup({ network: { label: "family", id_hex: "n1" } });
    w.h.notice.value = "Network \"family\" created.";
    w.store.state = withState({ network: { label: "family", id_hex: "n1" } });
    await nextTick();
    assert.equal(w.h.notice.value, "Network \"family\" created.");
    w.unmount();
  });
});

describe("useHome > \"Choose another\"", () => {
  it("focus goes to a member who is online, not to the failed chosen exit", async () => {
    const w = setup({ peers: [PEER("B", false), PEER("C", true)] });
    w.store.config = withConfig({ exits: ["B".repeat(64)] });
    await nextTick();
    w.h.pickOther();
    assert.deepEqual(w.focused, ['input.exit-choice[data-id="' + "C".repeat(64) + '"]']);
    w.unmount();
  });

  it("no other member online — focus goes to \"As usual\"", async () => {
    const w = setup({ peers: [PEER("B", false)] });
    w.store.config = withConfig({ exits: ["B".repeat(64)] });
    await nextTick();
    w.h.pickOther();
    assert.deepEqual(w.focused, ["#exit-direct"]);
    w.unmount();
  });
});

describe("useHome > tab", () => {
  it("without a choice — by mode; a click wins; \"Done\" resets the choice", async () => {
    const w = setup();
    assert.equal(w.h.activeTab.value, "main");
    w.h.setMark("browser");
    assert.equal(w.h.activeTab.value, "wizard");
    w.h.homeTab.value = "main";
    assert.equal(w.h.activeTab.value, "main");
    w.h.setMark("done");
    assert.equal(w.h.homeTab.value, null);
    assert.equal(w.h.activeTab.value, "main");
    w.unmount();
  });
});

describe("useHome > browser connection via PAC", () => {
  describe("system not chosen", () => {
    it("form.browser.os is empty, proxy.os — by userAgent", () => {
      const w = setup(PROXY_ON, { userAgent: UA_MAC });
      assert.equal(w.h.form.browser.os, null);
      assert.equal(w.h.proxy.value.os, "mac");
      w.unmount();
    });
  });

  describe("pickOs", () => {
    it("writes form.browser.os, proxy.os and the pac block — of the chosen system, regardless of userAgent", () => {
      const w = setup(PROXY_ON, { userAgent: UA_MAC });
      w.h.pickOs("linux");
      assert.equal(w.h.form.browser.os, "linux");
      assert.equal(w.h.proxy.value.os, "linux");
      assert.deepEqual(w.h.proxy.value.pac, pacBlock("linux"));
      w.unmount();
    });

    it("\"phone\" on a computer — a placeholder without steps", () => {
      const w = setup(PROXY_ON, { userAgent: UA_MAC });
      w.h.pickOs("phone");
      assert.equal(w.h.proxy.value.os, "phone");
      assert.deepEqual(w.h.proxy.value.pac, pacBlock("phone"));
      w.unmount();
    });

    it("choosing another system again replaces the previous one", () => {
      const w = setup(PROXY_ON, { userAgent: UA_MAC });
      w.h.pickOs("windows");
      w.h.pickOs("mac");
      assert.equal(w.h.form.browser.os, "mac");
      assert.equal(w.h.proxy.value.os, "mac");
      w.unmount();
    });
  });

  describe("copy", () => {
    it("\"pac\" puts proxy.pacUrl into the clipboard and shows \"Copied\" at pac", async () => {
      const w = setup(PROXY_ON, { clipboard: true });
      assert.equal(await w.h.copy("pac"), "copied");
      assert.equal(w.written.length, 1);
      assert.equal(w.written[0], w.h.proxy.value.pacUrl);
    assert.equal(w.written[0], "http://127.0.0.1:1080/node.pac", "pac_url from the snapshot, not built from listen");
      assert.equal(w.h.form.browser.copied, "pac");
      w.unmount();
    });

    it("\"host\" and \"port\" still put the proxy address and port", async () => {
      const w = setup(PROXY_ON, { clipboard: true });
      await w.h.copy("host");
      await w.h.copy("port");
      assert.deepEqual(w.written, ["127.0.0.1", "1080"]);
      w.unmount();
    });

    it("\"pac\" without a clipboard — \"fallback\"", async () => {
      const w = setup(PROXY_ON, {});
      assert.equal(await w.h.copy("pac"), "fallback");
      assert.equal(w.h.form.browser.noteKind, "pac");
      w.unmount();
    });
  });

  describe("toggleBrowser — manual block", () => {
    it("initially collapsed; toggles form.browser.open back and forth", () => {
      const w = setup(PROXY_ON, {});
      assert.equal(w.h.form.browser.open, false);
      w.h.toggleBrowser();
      assert.equal(w.h.form.browser.open, true);
      w.h.toggleBrowser();
      assert.equal(w.h.form.browser.open, false);
      w.unmount();
    });
  });
});

const UA_MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/node.pac" } };

const PEER = (ch, online) => ({ id: ch.repeat(10), id_full: ch.repeat(64), name: ch, online, exit: true });

function withState(over = {}) {
  return {
    node: { status: "online", id: "aaaa1111" },
    network: { label: "home", id_hex: "n0" },
    peers: [{ self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "old-name" }],
    ...over,
  };
}

function withConfig({ name = "old-name", exits = [] } = {}) {
  return { node: { name }, route: { exits }, exit: { enabled: false } };
}

// Preconditions: a network in the snapshot (`network` and `socks` override; null — no network), node name "old-name";
// the browser's `userAgent`; `clipboard: true` — the clipboard writes to `written`, otherwise there is no clipboard.
function setup(state = {}, { userAgent = "", clipboard = false } = {}) {
  const store = createStore();
  store.state = withState(state);
  store.config = withConfig();
  store.configRev = 1;
  const written = [];
  const deps = {
    api: async () => ({}),
    clipboard: clipboard ? { writeText: async (t) => { written.push(t); } } : null,
    setTimer: () => 1,
    clearTimer: () => {},
    storage: { getItem: () => null, setItem: () => {} },
    userAgent,
    focus: (selector) => focused.push(selector),
    askKeep: async () => "no",
  };
  const focused = [];
  const { result: h, unmount } = withSetup(() => useHome(store, deps));
  return { h, store, unmount, focused, written };
}
