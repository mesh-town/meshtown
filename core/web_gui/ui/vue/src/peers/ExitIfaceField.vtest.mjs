/* The field "Send exit traffic through" in the Device settings block of a network tab (`ExitIfaceField`), checked on the whole page
 * on a fake node with one network. Contract: the list is `#you-exit-iface` (a `select`; the value "" is "any network"), the warning
 * about the chosen interface is `#you-exit-iface-state`, the status of saving is `#you-exit-iface-status`. The field sits next to
 * the switch `#you-exit-toggle`: shown only when the network lets members open sites, never in the phone app. A choice is saved
 * as `exit.interface` to the config of its network; "any network" is saved as `null`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_A, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig } from "../testing/section.mjs";

const REFUSED_TEXT = "Local port 5353 is already in use";
const MISSING_TEXT = "wg9 is not on this computer — exit connections are refused";
const NETWORKS = [
  { name: "en0", addrs: ["192.168.1.10"], default_route: true, present: true, open: false, by: "auto" },
  { name: "wg0", addrs: ["10.8.0.2"], default_route: false, kind: "tunnel", present: true, open: false, by: "auto" },
];

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("exit is on", () => {
  it("shows the list next to the switch", async () => {
    const page = await open();
    expect(page.field().exists()).toBe(true);
    expect(page.wrapper.find("#you-exit-toggle").exists()).toBe(true);
  });

  it("offers any network first, then the networks of this computer with tunnels first", async () => {
    const page = await open();
    expect(page.optionValues()).toEqual(["", "wg0", "en0"]);
  });

  it("no choice in the config: any network is selected", async () => {
    const page = await open();
    expect(page.field().element.value).toBe("");
  });

  it("a saved choice is selected", async () => {
    const page = await open({ iface: "wg0" });
    expect(page.field().element.value).toBe("wg0");
  });

  it("says nothing under the field while the chosen interface is up", async () => {
    const page = await open({ iface: "wg0", info: { name: "wg0", state: "up" } });
    expect(page.stateText()).toBe("");
  });

  it("says nothing under the field when the node reports no interface", async () => {
    const page = await open({ iface: null, info: null });
    expect(page.stateText()).toBe("");
  });

  it("warns under the field when the chosen interface is missing", async () => {
    const page = await open({ iface: "wg9", info: { name: "wg9", state: "missing" } });
    expect(page.stateText()).toBe(MISSING_TEXT);
  });
});

describe("hidden when exit is off", () => {
  it("has no list", async () => {
    const page = await open({ exitOn: false });
    expect(page.field().exists()).toBe(false);
  });

  it("has no state line and no status either", async () => {
    const page = await open({ exitOn: false, iface: "wg9", info: { name: "wg9", state: "missing" } });
    expect(page.wrapper.find("#you-exit-iface-state").exists()).toBe(false);
    expect(page.wrapper.find("#you-exit-iface-status").exists()).toBe(false);
  });
});

describe("in the phone app", () => {
  beforeEach(() => Object.defineProperty(navigator, "userAgent", { value: "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android", configurable: true }));
  afterEach(() => Reflect.deleteProperty(navigator, "userAgent"));

  it("has no list", async () => {
    const page = await open();
    expect(page.field().exists()).toBe(false);
  });
});

describe("saves choice", () => {
  it("a network picked: sends exit.interface with its name, once, to the config", async () => {
    const page = await open();
    await page.whenPicked("wg0");
    expect(page.puts()).toHaveLength(1);
    expect(page.puts()[0].config.exit.interface).toBe("wg0");
  });

  it("a network picked: says that it is saved", async () => {
    const page = await open();
    await page.whenPicked("wg0");
    expect(page.status()).toBe("Saved");
  });

  it("a network picked: keeps it selected", async () => {
    const page = await open();
    await page.whenPicked("wg0");
    expect(page.field().element.value).toBe("wg0");
  });
});

describe("any network sends null", () => {
  it("after a network was saved, picking any network sends exit.interface as null", async () => {
    const page = await open({ iface: "wg0" });
    await page.whenPicked("");
    expect(page.puts()).toHaveLength(1);
    expect(page.puts()[0].config.exit.interface).toBeNull();
  });

  it("says that it is saved", async () => {
    const page = await open({ iface: "wg0" });
    await page.whenPicked("");
    expect(page.status()).toBe("Saved");
  });
});

describe("shows field error", () => {
  it("the node refuses the name: its text for the field is in the status", async () => {
    const page = await open({ refuse: true });
    await page.whenPicked("wg0");
    expect(page.status()).toBe(REFUSED_TEXT);
  });

  it("does not say that it is saved", async () => {
    const page = await open({ refuse: true });
    await page.whenPicked("wg0");
    expect(page.status()).not.toBe("Saved");
  });

  it("the next successful save replaces the error with \"Saved\"", async () => {
    const page = await open({ refuse: true });
    await page.whenPicked("wg0");
    delete page.node.routes["PUT /api/config"];
    await page.whenPicked("en0");
    expect(page.status()).toBe("Saved");
  });
});

describe("disabled when config is not writable", () => {
  it("the command line fixes exit.interface: the list is disabled", async () => {
    const page = await open({ readOnly: ["exit.interface"] });
    expect(page.field().attributes("disabled")).toBeDefined();
  });

  it("nothing is fixed: the list is enabled", async () => {
    const page = await open();
    expect(page.field().attributes("disabled")).toBeUndefined();
  });

  it("only the switch is fixed: the list stays enabled", async () => {
    const page = await open({ readOnly: ["exit.enabled"] });
    expect(page.field().attributes("disabled")).toBeUndefined();
  });
});

// The state is the main network's snapshot with the networks of this computer and the report on the exit interface.
async function open({ exitOn = true, iface, info = null, readOnly = [], refuse = false } = {}) {
  const opts = twoNets();
  const exit = iface === undefined ? { enabled: exitOn } : { enabled: exitOn, interface: iface };
  opts.config = baseConfig({ exit });
  opts.state = { ...opts.state, exit: { enabled: exitOn, interface: info }, node: { ...opts.state.node, networks: NETWORKS } };
  const node = createFakeNode({ ...opts, slots: [opts.slots[0]] });
  if (readOnly.length > 0) {
    node.routes["GET /api/config"] = () => ({ rev: node.rev, config: structuredClone(node.config), file: null, read_only: readOnly, restart_fields: [], restart_pending: [], cli_overrides: [] });
  }
  if (refuse) {
    node.routes["PUT /api/config"] = { status: 422, body: { error: "invalid", message: "not allowed", fields: [{ path: "exit.interface", message: REFUSED_TEXT }] } };
  }
  return pageOf(node, await mountPage(node, { hash: "peers/" + NET_A + "/device" }));
}

function pageOf(node, { wrapper, store }) {
  const field = () => wrapper.find("#you-exit-iface");
  const textOf = (selector) => (wrapper.find(selector).exists() ? wrapper.get(selector).text().replace(/\s+/g, " ").trim() : "");
  return {
    node, wrapper, store, field,
    optionValues: () => field().findAll("option").map((o) => o.element.value),
    stateText: () => textOf("#you-exit-iface-state"),
    status: () => textOf("#you-exit-iface-status"),
    puts: () => node.calls.filter((c) => c.method === "PUT" && c.path === "/api/config").map((c) => c.body),
    async whenPicked(value) {
      await field().setValue(value);
      await vi.advanceTimersByTimeAsync(0);
      await settle();
    },
  };
}
