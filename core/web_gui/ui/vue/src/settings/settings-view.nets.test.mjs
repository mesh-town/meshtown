/* "Settings" over several networks: which edits are shared, which networks need a restart, captions,
 * the restart button of the save bar. The output reads as a phrase: `restartTargets > no_restart_slots > ...`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { createStore } from "../core/store.ts";
import { EXPERT_FIELDS, MAIN_FIELDS, PAGE_FIELDS, SHARED_PATHS } from "./settings-fields.ts";
import { fieldCaption, fieldLabel, restartTargets, saveBarView, sharedChanges } from "./settings-view.ts";

const B = "k7q2xa";
const CAPTION = "Applies to all your networks.";
const MULTI_WEB_ACCESS = "Which connections of this computer can open this page (Wi-Fi, cable)";

describe("sharedChanges", () => {
  describe("the shared paths", () => {
    it("are the checks and the probe interval", () => {
      assert.deepEqual([...SHARED_PATHS].sort(), [
        "checks.doh_server", "checks.enabled", "checks.interval_sec", "checks.provider_dns", "probe.interval_sec",
      ]);
    });

    it("each one passes through with its value", () => {
      const changes = Object.fromEntries(SHARED_PATHS.map((path, i) => [path, `v${i}`]));
      assert.deepEqual(sharedChanges(changes), changes);
    });
  });

  describe("mixed edit", () => {
    it("only the shared paths remain", () => {
      const changes = { "checks.interval_sec": 900, "socks.port": 1090, "web.access": "localnet" };
      assert.deepEqual(sharedChanges(changes), { "checks.interval_sec": 900 });
    });

    it("the edit itself is not modified", () => {
      const changes = { "checks.enabled": false, "socks.port": 1090 };
      sharedChanges(changes);
      assert.deepEqual(changes, { "checks.enabled": false, "socks.port": 1090 });
    });
  });

  describe("no shared path", () => {
    it("process paths only: an empty object", () => {
      assert.deepEqual(sharedChanges({ "socks.port": 1090, "socks.enabled": false }), {});
    });

    it("empty edit: an empty object", () => {
      assert.deepEqual(sharedChanges({}), {});
    });
  });

  describe("falsy values", () => {
    it("false, 0 and null of a shared path are kept", () => {
      const changes = { "checks.enabled": false, "checks.provider_dns": null, "probe.interval_sec": 0 };
      assert.deepEqual(sharedChanges(changes), changes);
    });
  });
});

describe("restartTargets", () => {
  describe("restart_slots is set", () => {
    it("the same slots in the same order", () => {
      assert.deepEqual(restartTargets(storeWith({ restartSlots: ["main", B] })), ["main", B]);
    });

    it("the other network alone: the main one is not added", () => {
      assert.deepEqual(restartTargets(storeWith({ restartSlots: [B] })), [B]);
    });

    it("wins over restart_pending of the main config", () => {
      assert.deepEqual(restartTargets(storeWith({ restartSlots: [B], pending: ["socks.port"] })), [B]);
    });
  });

  describe("restart_slots is empty", () => {
    it("restart_pending is set: only the main one", () => {
      assert.deepEqual(restartTargets(storeWith({ pending: ["socks.port"] })), ["main"]);
    });

    it("restart_pending is empty: nothing", () => {
      assert.deepEqual(restartTargets(storeWith({})), []);
    });

    it("no config meta at all: nothing", () => {
      assert.deepEqual(restartTargets(storeWith({ meta: null })), []);
    });
  });
});

describe("fieldCaption", () => {
  describe("two networks", () => {
    it("every row of Advanced says it applies to all networks", () => {
      for (const field of EXPERT_FIELDS) assert.equal(fieldCaption(field, true), CAPTION, field.path);
    });

    it("every row of Page says it applies to all networks", () => {
      for (const field of PAGE_FIELDS) assert.equal(fieldCaption(field, true), CAPTION, field.path);
    });

    it("a row of the main block has no caption", () => {
      for (const field of MAIN_FIELDS) assert.equal(fieldCaption(field, true), "", field.path);
    });
  });

  describe("one network", () => {
    it("no row has a caption", () => {
      for (const field of [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS]) assert.equal(fieldCaption(field, false), "", field.path);
    });
  });
});

describe("fieldLabel", () => {
  describe("two networks", () => {
    it("web.access asks which connections of this computer can open the page", () => {
      assert.equal(fieldLabel(fieldOf("web.access"), true), MULTI_WEB_ACCESS);
    });

    it("another row keeps its title", () => {
      const field = fieldOf("socks.port");
      assert.equal(fieldLabel(field, true), field.title);
    });
  });

  describe("one network", () => {
    it("web.access keeps its title", () => {
      const field = fieldOf("web.access");
      assert.equal(fieldLabel(field, false), field.title);
      assert.notEqual(fieldLabel(field, false), MULTI_WEB_ACCESS);
    });
  });
});

describe("saveBarView restart button", () => {
  describe("restartTargets is set", () => {
    it("the button is offered", () => {
      assert.equal(bar({ restartTargets: [B] }).restartOffered, true);
    });

    it("the button is offered even when the main config has no restart_pending", () => {
      assert.equal(bar({ restartTargets: ["main", B], pending: [] }).restartOffered, true);
    });

    it("the restart has been asked already: no button", () => {
      assert.equal(bar({ restartTargets: ["main"], requested: true }).restartOffered, false);
    });
  });

  describe("restartTargets is empty", () => {
    it("no button", () => {
      assert.equal(bar({ restartTargets: [] }).restartOffered, false);
    });

    it("no button even when the main config has restart_pending", () => {
      assert.equal(bar({ restartTargets: [], pending: ["socks.port"] }).restartOffered, false);
    });
  });
});

function fieldOf(path) {
  const field = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].find((f) => f.path === path);
  assert.ok(field, "no field " + path);
  return field;
}

function storeWith({ restartSlots = [], pending = [], meta }) {
  const store = createStore();
  store.restartSlots = restartSlots;
  store.configMeta = meta === undefined ? { read_only: [], restart_fields: [], restart_pending: pending, cli_overrides: [] } : meta;
  return store;
}

function bar({ restartTargets, pending = [], requested = false }) {
  return saveBarView({
    dirty: false,
    meta: { read_only: [], restart_fields: [], restart_pending: pending, cli_overrides: [] },
    configFile: null,
    restartTargets,
    requested,
    busy: false,
    message: { text: "", cls: "hint" },
  });
}
