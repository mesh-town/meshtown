/* "Settings" on a node with two networks (A = `main`, B = "my devices"): shared fields are written to every network that
 * holds a key, the main config first; a refusal of B keeps the fields unsaved; restart goes to each network that waits for it;
 * the network card offers "Add another network" instead of the join form. With one network the page keeps its former requests.
 * The whole page runs on a fake node. */

import { afterEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import { SETTINGS_TITLE_CHECKS_INTERVAL, SETTINGS_TITLE_PROBE_INTERVAL, SETTINGS_TITLE_SOCKS_PORT } from "./settings-texts.ts";

const MAIN_CONFIG = "/api/config";
const B_CONFIG = `/api/net/${NET_B}/config`;
const MAIN_RESTART = "/api/actions/restart";
const B_RESTART = `/api/net/${NET_B}/actions/restart`;
const B_LABEL = "my devices";
const INTERVAL = "900";
const NEW_PORT = "1081";
const REFUSAL = "disk is full";
const NAME_PER_NET = "Your name is set for each network: Members → the network's tab.";
const CAPTION = "Applies to all your networks.";
const PARTIAL = "some networks did not accept it";

afterEach(unmountPage);

describe("SettingsSection.nets", () => {
  describe("two networks, \"How often to check sites\" is changed and saved", () => {
    it("writes the whole main config first, then the config of B", async () => {
      const { node, wrapper } = await setup();
      await editInterval(wrapper);
      await save(wrapper);
      expect(puts(node).map((c) => c.path)).toEqual([MAIN_CONFIG, B_CONFIG]);
      expect(puts(node)[0].body.config.checks.interval_sec).toBe(Number(INTERVAL));
      expect(puts(node)[0].body.config.ports, "the rest of the main config is kept").toEqual(twoNets().config.ports);
    });

    it("sends B its own config with that single path changed", async () => {
      const { node, wrapper } = await setup();
      await editInterval(wrapper);
      await save(wrapper);
      const expected = { ...twoNets().slots[1].config, checks: { interval_sec: Number(INTERVAL) } };
      expect(puts(node)[1].body.config).toEqual(expected);
    });

    it("sends B only the shared path when a process field is edited together with it", async () => {
      const { node, wrapper } = await setup();
      await editInterval(wrapper);
      await editPort(wrapper);
      await save(wrapper);
      expect(puts(node)[0].body.config.socks.port, "main gets the port").toBe(Number(NEW_PORT));
      expect(puts(node)[1].body.config.socks, "B does not").toBeUndefined();
    });

    it("reports no refusal and leaves nothing unsaved", async () => {
      const { node, wrapper } = await setup();
      await editInterval(wrapper);
      await save(wrapper);
      expect(node.unexpected).toEqual([]);
      expect(message(wrapper)).not.toContain(PARTIAL);
      expect(tabDirty(wrapper), "nothing left unsaved").toBe(false);
    });
  });

  describe("two networks, only a process field is changed", () => {
    it("writes the main config and no slot config", async () => {
      const { node, wrapper } = await setup();
      await editPort(wrapper);
      await save(wrapper);
      expect(puts(node).map((c) => c.path)).toEqual([MAIN_CONFIG]);
    });
  });

  describe("B refuses the write", () => {
    it("keeps the main config written and names B in the message", async () => {
      const { node, wrapper } = await setup();
      refuseB(node);
      await editInterval(wrapper);
      await save(wrapper);
      expect(puts(node).map((c) => c.path)).toEqual([MAIN_CONFIG, B_CONFIG]);
      expect(message(wrapper)).toContain(PARTIAL);
      expect(message(wrapper)).toContain(`"${B_LABEL}": ${REFUSAL}`);
    });

    it("leaves the field unsaved", async () => {
      const { node, wrapper } = await setup();
      refuseB(node);
      await editInterval(wrapper);
      await save(wrapper);
      expect(tabDirty(wrapper), "the tab still shows unsaved changes").toBe(true);
      expect(intervalSelect(wrapper).element.value).toBe(INTERVAL);
    });

    it("sends both writes again on the next Save", async () => {
      const { node, wrapper } = await setup();
      refuseB(node);
      await editInterval(wrapper);
      await save(wrapper);
      delete node.routes[`PUT ${B_CONFIG}`];
      const before = node.calls.length;
      await save(wrapper);
      expect(puts(node, before).map((c) => c.path)).toEqual([MAIN_CONFIG, B_CONFIG]);
      expect(puts(node, before)[1].body.config.checks.interval_sec).toBe(Number(INTERVAL));
      expect(tabDirty(wrapper)).toBe(false);
      expect(message(wrapper)).not.toContain(PARTIAL);
    });
  });

  describe("B has a newer config revision", () => {
    it("rereads the config of B once and repeats the write once", async () => {
      const { node, wrapper } = await setup();
      node.slotList[1].rev = 7;
      await editInterval(wrapper);
      const before = node.calls.length;
      await save(wrapper);
      const later = node.calls.slice(before);
      const writes = puts(node, before).filter((c) => c.path === B_CONFIG);
      expect(later.filter((c) => c.method === "GET" && c.path === B_CONFIG)).toHaveLength(1);
      expect(writes).toHaveLength(2);
      expect(writes[1].body.rev, "the retry carries the fresh revision").toBe(7);
      expect(message(wrapper)).not.toContain(PARTIAL);
    });
  });

  describe("B holds no key yet", () => {
    it("writes the main config and no slot config", async () => {
      const { node, wrapper } = await setup({ nets: twoNets({ id_hex: null }), before: (n) => { n.slotList[1].joining = true; } });
      await editInterval(wrapper);
      await save(wrapper);
      expect(puts(node).map((c) => c.path)).toEqual([MAIN_CONFIG]);
    });
  });

  describe("restart", () => {
    it("asks the main network first and then B when both wait for it", async () => {
      const { node, wrapper } = await setup({ before: waitingForRestart });
      expect(restartShown(wrapper), "the button is offered").toBe(true);
      await wrapper.get("#restart-node-btn").trigger("click");
      await settle();
      expect(restarts(node)).toEqual([MAIN_RESTART, B_RESTART]);
    });

    it("asks only B when only B waits for it", async () => {
      const { node, wrapper } = await setup({ before: (n) => { n.slotList[1].pending = ["socks.ifaces"]; } });
      await wrapper.get("#restart-node-btn").trigger("click");
      await settle();
      expect(restarts(node)).toEqual([B_RESTART]);
    });

    it("has no button when no network waits and nothing is pending", async () => {
      const { node, wrapper } = await setup();
      expect(restartShown(wrapper)).toBe(false);
      expect(restarts(node)).toEqual([]);
    });
  });

  describe("the network card", () => {
    it("offers \"Add another network\" and no join form when there are two networks", async () => {
      const { wrapper } = await setup();
      expect(section(wrapper).find("#add-net-card").exists(), "add-network card").toBe(true);
      expect(section(wrapper).find("#join-card").exists(), "join card").toBe(false);
    });

    it("does the same when there is one network", async () => {
      const { wrapper } = await setup({ nets: oneNet() });
      expect(section(wrapper).find("#add-net-card").exists(), "add-network card").toBe(true);
      expect(section(wrapper).find("#join-card").exists(), "join card").toBe(false);
    });
  });

  describe("the node name", () => {
    it("is replaced by a pointer to Members when there are two networks", async () => {
      const { wrapper } = await setup();
      expect(section(wrapper).find("#settings-node-name").exists(), "name field").toBe(false);
      expect(flat(section(wrapper))).toContain(NAME_PER_NET);
    });

    it("is an ordinary field when there is one network", async () => {
      const { wrapper } = await setup({ nets: oneNet() });
      expect(section(wrapper).find("#settings-node-name").exists(), "name field").toBe(true);
      expect(flat(section(wrapper))).not.toContain(NAME_PER_NET);
    });
  });

  describe("the caption under the Advanced fields", () => {
    it("says the value applies to all networks, under \"How often to check connection speed\", when there are two networks", async () => {
      const { wrapper } = await setup();
      expect(rowText(wrapper, SETTINGS_TITLE_PROBE_INTERVAL)).toContain(CAPTION);
    });

    it("is absent when there is one network", async () => {
      const { wrapper } = await setup({ nets: oneNet() });
      expect(flat(section(wrapper))).not.toContain(CAPTION);
    });
  });
});

async function setup({ nets = twoNets(), before } = {}) {
  const node = createFakeNode(nets);
  before?.(node);
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "settings");
  return { node, wrapper };
}

function oneNet() {
  return { state: baseState(), config: baseConfig() };
}

function waitingForRestart(node) {
  node.pending = ["socks.ifaces"];
  node.slotList[1].pending = ["socks.ifaces"];
}

function refuseB(node) {
  node.routes[`PUT ${B_CONFIG}`] = { status: 500, body: { error: "write_failed", message: REFUSAL } };
}

const section = (wrapper) => wrapper.get("#sec-settings");
const flat = (wrapper) => wrapper.text().replace(/\s+/g, " ").replace(/’/g, "'").trim();
const message = (wrapper) => wrapper.get("#settings-save-message").text();
const tabDirty = (wrapper) => wrapper.get("#tab-settings").classes().includes("has-dirty");
const intervalSelect = (wrapper) => wrapper.get(`select[aria-label="${SETTINGS_TITLE_CHECKS_INTERVAL}"]`);
const restartShown = (wrapper) => wrapper.find("#restart-node-btn").exists() && shown(wrapper.get("#restart-node-btn").element);
const puts = (node, from = 0) => node.calls.slice(from).filter((c) => c.method === "PUT");
const restarts = (node) => node.calls.filter((c) => c.method === "POST" && c.path.endsWith("/actions/restart")).map((c) => c.path);

async function editInterval(wrapper) {
  await intervalSelect(wrapper).setValue(INTERVAL);
  await settle();
}

async function editPort(wrapper) {
  await wrapper.get(`input[aria-label="${SETTINGS_TITLE_SOCKS_PORT}"]`).setValue(NEW_PORT);
  await settle();
}

async function save(wrapper) {
  await wrapper.get("#settings-save-btn").trigger("click");
  await settle();
}

// The text of a field row and of the caption that follows it as a separate element, if any.
function rowText(wrapper, title) {
  const row = wrapper.get(`[aria-label="${title}"]`).element.closest(".setting-row");
  const next = row.nextElementSibling;
  const caption = next && !next.classList.contains("setting-row") ? next.textContent : "";
  return (row.textContent + " " + caption).replace(/\s+/g, " ");
}
