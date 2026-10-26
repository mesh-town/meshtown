/* The "You in <network>" block, the Device settings section of a network tab: the whole page on a fake node with two networks.
 * Network A is `main` ("friends"), this computer is called "alice-box" in it; network B is `k7q2xa` ("my devices"), the name there is
 * "bob-box". Contract: the block is `#you-block` (the Device settings section, `#peers/<slot>/device`, also with one network), the name field `#you-name`, the link that copies the name of the
 * first network `#you-same-name` (not on the first network), the switch "let members open sites" `#you-exit-toggle`.
 * Everything the block saves goes to the address of its own network and never to `PUT /api/config`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig } from "../testing/section.mjs";

const PUT_B = "/api/net/" + NET_B + "/config";
const NAME_A = "alice-box";
const NAME_B = "bob-box";
const STALE = "settings were changed in another tab";
const ON_TEXT = 'On: members of the network "my devices" can open sites through your internet connection, and sites will see your address.';
const OFF_TEXT = `Off: members of the network "my devices" can't open sites through your internet connection.`;

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("on the first network", () => {
  it("is titled with the name of the network", async () => {
    const page = await openTwo({ hash: "peers/main/device" });
    expect(page.title()).toBe('You in "friends"');
  });

  it("shows the name of this computer in that network", async () => {
    const page = await openTwo({ hash: "peers/main/device" });
    expect(page.nameField().element.value).toBe(NAME_A);
  });

  it("has no link to use the name of the first network", async () => {
    const page = await openTwo({ hash: "peers/main/device" });
    expect(page.sameNameLink().exists()).toBe(false);
  });
});

describe("on the second network", () => {
  it("is titled with the name of the network", async () => {
    const page = await openTwo();
    expect(page.title()).toBe('You in "my devices"');
  });

  it("shows the name of this computer in that network, not in the first", async () => {
    const page = await openTwo();
    expect(page.nameField().element.value).toBe(NAME_B);
  });

  it("offers to use the name of the first network", async () => {
    const page = await openTwo();
    expect(page.sameNameLink().text()).toBe('Use the same name as in "friends"');
  });
});

describe("one network", () => {
  it("has the block in Device settings, with the name of this computer", async () => {
    const page = await openOne();
    expect(page.block().exists()).toBe(true);
    expect(page.nameField().element.value).toBe(NAME_A);
  });

  it("one network: no same-name link", async () => {
    const page = await openOne();
    expect(page.sameNameLink().exists()).toBe(false);
  });

  it("still offers the switch and \"Leave network…\"", async () => {
    const page = await openOne();
    expect(page.exitToggle().exists()).toBe(true);
    expect(page.leaveButton()).toBeTruthy();
  });
});

describe("in the phone app", () => {
  beforeEach(() => setUa("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android"));
  afterEach(() => Reflect.deleteProperty(navigator, "userAgent"));

  it("app: no exit toggle, no caption and no status of it", async () => {
    const page = await openTwo();
    expect(page.exitToggle().exists()).toBe(false);
    expect(page.wrapper.find("#you-exit-hint").exists()).toBe(false);
    expect(page.wrapper.find("#you-exit-status").exists()).toBe(false);
  });

  it("still has the name field and the way out", async () => {
    const page = await openTwo();
    expect(page.nameField().exists()).toBe(true);
    expect(page.leaveButton()).toBeTruthy();
  });
});

describe("the block lives in Device settings only", () => {
  it("is not on Members", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    expect(page.block().exists()).toBe(false);
  });

  it("is not on Shared services", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B + "/services" });
    expect(page.block().exists()).toBe(false);
  });
});

describe("the link to use the name of the first network", () => {
  it("puts that name into the field", async () => {
    const page = await openTwo();
    await page.whenSameNameClicked();
    expect(page.nameField().element.value).toBe(NAME_A);
  });

  it("saves it once, to the second network only", async () => {
    const page = await openTwo();
    await page.whenSameNameClicked();
    expect(page.puts(PUT_B)).toHaveLength(1);
    expect(page.puts(PUT_B)[0].config.node.name).toBe(NAME_A);
    expect(page.puts("/api/config"), "nothing for the first network").toEqual([]);
  });

  it("says that the name is saved", async () => {
    const page = await openTwo();
    await page.whenSameNameClicked();
    expect(page.nameStatus()).toBe("Saved");
  });
});

describe("the name field", () => {
  describe("a new name and Enter", () => {
    it("saves it once, to the address of this network", async () => {
      const page = await openTwo();
      await page.whenNameEntered("night-owl");
      expect(page.puts(PUT_B)).toHaveLength(1);
      expect(page.puts(PUT_B)[0].config.node.name).toBe("night-owl");
    });

    it("does not touch the config of the first network", async () => {
      const page = await openTwo();
      await page.whenNameEntered("night-owl");
      expect(page.puts("/api/config")).toEqual([]);
    });

    it("says that the name is saved and keeps it in the field", async () => {
      const page = await openTwo();
      await page.whenNameEntered("night-owl");
      expect(page.nameStatus()).toBe("Saved");
      expect(page.nameField().element.value).toBe("night-owl");
    });
  });

  describe("the same name and Enter", () => {
    it("sends nothing", async () => {
      const page = await openTwo();
      await page.whenNameEntered(NAME_B);
      expect(page.puts(PUT_B)).toEqual([]);
      expect(page.puts("/api/config")).toEqual([]);
    });
  });

  describe("an empty name and Enter", () => {
    it("saves an empty string, which means a random name", async () => {
      const page = await openTwo();
      await page.whenNameEntered("");
      expect(page.puts(PUT_B)).toHaveLength(1);
      expect(page.puts(PUT_B)[0].config.node.name).toBe("");
    });
  });

  describe("the node says the settings changed elsewhere", () => {
    it("shows the reason next to the field", async () => {
      const page = await openTwo({ staleOnSave: true });
      await page.whenNameEntered("night-owl");
      expect(page.nameStatus()).toBe("Couldn't save: " + STALE);
    });

    it("does not say that the name is saved", async () => {
      const page = await openTwo({ staleOnSave: true });
      await page.whenNameEntered("night-owl");
      expect(page.nameStatus()).not.toBe("Saved");
    });
  });

  describe("the name is fixed by the command line", () => {
    it("the field is disabled", async () => {
      const page = await openTwo({ readOnly: ["node.name"] });
      expect(page.nameField().attributes("disabled")).toBeDefined();
    });

    it("the switch stays enabled", async () => {
      const page = await openTwo({ readOnly: ["node.name"] });
      expect(page.exitToggle().attributes("disabled")).toBeUndefined();
    });
  });

  it("is enabled when nothing is fixed", async () => {
    const page = await openTwo();
    expect(page.nameField().attributes("disabled")).toBeUndefined();
  });
});

describe("the switch \"let members open sites through this computer\"", () => {
  it("is on when the network lets members through, with the \"On\" caption", async () => {
    const page = await openTwo();
    expect(page.exitToggle().element.checked).toBe(true);
    expect(page.exitHint()).toBe(ON_TEXT);
  });

  it("turned off, saves exit.enabled=false to this network only", async () => {
    const page = await openTwo();
    await page.whenExitSwitched(false);
    expect(page.puts(PUT_B)).toHaveLength(1);
    expect(page.puts(PUT_B)[0].config.exit.enabled).toBe(false);
    expect(page.puts("/api/config"), "nothing for the first network").toEqual([]);
  });

  it("turned off, changes the caption to \"Off\"", async () => {
    const page = await openTwo();
    await page.whenExitSwitched(false);
    expect(page.exitHint()).toBe(OFF_TEXT);
  });

  it("turned off and on again, brings the \"On\" caption back", async () => {
    const page = await openTwo();
    await page.whenExitSwitched(false);
    await page.whenExitSwitched(true);
    expect(page.puts(PUT_B).at(-1).config.exit.enabled).toBe(true);
    expect(page.exitHint()).toBe(ON_TEXT);
  });

  it("is disabled when the command line fixes it", async () => {
    const page = await openTwo({ readOnly: ["exit.enabled"] });
    expect(page.exitToggle().attributes("disabled")).toBeDefined();
  });
});

describe("\"Leave network…\"", () => {
  it("opens the window that asks to leave the second network", async () => {
    const page = await openTwo();
    await page.whenLeaveClicked();
    expect(page.dialogTitle()).toBe('Leave "my devices"?');
  });

  it("says in the window that the other networks stay", async () => {
    const page = await openTwo();
    await page.whenLeaveClicked();
    expect(page.dialogBody()).toContain("Your other networks stay.");
  });

  it("on the first tab asks to leave the first network", async () => {
    const page = await openTwo({ hash: "peers/main/device" });
    await page.whenLeaveClicked();
    expect(page.dialogTitle()).toBe('Leave "friends"?');
  });

  it("leaves nothing sent until the window is confirmed", async () => {
    const page = await openTwo();
    await page.whenLeaveClicked();
    expect(page.node.calls.filter((c) => c.method === "POST")).toEqual([]);
  });
});

const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });
const compact = (text) => text.replace(/\s+/g, " ").trim();
const withNames = (opts) => {
  opts.config = baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] }, node: { name: NAME_A }, exit: { enabled: true } });
  return opts;
};

function networkB() {
  const config = baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] }, node: { name: NAME_B }, exit: { enabled: true } });
  return { config };
}

// The slot answers its config with `read_only` and refuses every save with a stale revision, when the test says so.
function arrange(node, { readOnly, staleOnSave }) {
  const b = node.slotList.find((s) => s.id === NET_B);
  if (readOnly.length > 0) {
    b.routes["GET /api/config"] = () => ({ rev: b.rev, config: structuredClone(b.config), file: null, read_only: readOnly, restart_fields: [], restart_pending: [], cli_overrides: [] });
  }
  if (staleOnSave) {
    b.routes["PUT /api/config"] = { status: 409, body: { error: "stale_rev", message: STALE, rev: b.rev, config: structuredClone(b.config) } };
  }
}

async function openTwo({ hash = "peers/" + NET_B + "/device", readOnly = [], staleOnSave = false } = {}) {
  const node = createFakeNode(withNames(twoNets(networkB())));
  arrange(node, { readOnly, staleOnSave });
  return pageOf(node, await mountPage(node, { hash }));
}

async function openOne() {
  const opts = withNames(twoNets(networkB()));
  const node = createFakeNode({ ...opts, slots: [opts.slots[0]] });
  return pageOf(node, await mountPage(node, { hash: "peers/main/device" }));
}

function pageOf(node, { wrapper, store }) {
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const block = () => wrapper.find("#you-block");
  const leaveButton = () => block().findAll("button").find((b) => compact(b.text()) === "Leave network…");
  return {
    node, wrapper, store, block,
    title: () => compact(wrapper.get("#you-block h2").text()),
    nameField: () => wrapper.get("#you-name"),
    sameNameLink: () => wrapper.find("#you-same-name"),
    exitToggle: () => wrapper.find("#you-exit-toggle"),
    leaveButton,
    nameStatus: () => compact(wrapper.get("#you-name-status").text()),
    exitHint: () => compact(wrapper.get("#you-exit-hint").text()),
    dialogTitle: () => compact(wrapper.get("#confirm-title").text()),
    dialogBody: () => compact(wrapper.get("#confirm-body").text()),
    puts: (path) => node.calls.filter((c) => c.method === "PUT" && c.path === path).map((c) => c.body),
    whenSameNameClicked: () => click(wrapper.get("#you-same-name")),
    async whenNameEntered(name) {
      await wrapper.get("#you-name").setValue(name);
      await wrapper.get("#you-name").trigger("keydown", { key: "Enter" });
      await vi.advanceTimersByTimeAsync(0);
      await settle();
    },
    async whenExitSwitched(on) {
      await wrapper.get("#you-exit-toggle").setValue(on);
      await settle();
    },
    whenLeaveClicked: () => click(leaveButton()),
  };
}
