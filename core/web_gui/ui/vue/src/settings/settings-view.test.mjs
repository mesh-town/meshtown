/* What to show in "Settings": the field row, notes, the file banner, the save bar.
 * The field row, marks, the save bar. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../common/words.ts";
import { MAIN_FIELDS, EXPERT_FIELDS, PAGE_FIELDS } from "./settings-fields.ts";
import {
  metaHas,
  fieldView,
  cliNoteText,
  savedMessageText,
  fileBanner,
  saveBarView,
  RESTART_STARTED,
} from "./settings-view.ts";

describe("metaHas", () => {
  it("the path is in the meta list — true; absent or meta null — false", () => {
    const meta = meta_({ restart_fields: ["socks.port"] });
    assert.equal(metaHas(meta, "restart_fields", "socks.port"), true);
    assert.equal(metaHas(meta, "restart_fields", "web.port"), false);
    assert.equal(metaHas(null, "restart_fields", "socks.port"), false);
  });
});

describe("fieldView", () => {
  describe("read-only, boolean value", () => {
    it("true — a locked enabled switch, cannot be edited", () => {
      const v = viewOf("web.enabled", {});
      assert.equal(v.readOnly, true);
      assert.equal(v.disabled, true);
      assert.equal(v.editable, false);
      assert.equal(v.value, true);
      assert.equal(v.id, "field-web-enabled");
    });

    it("false — a disabled one", () => {
      const v = viewOf("web.enabled", { config: { web: { enabled: false, port: 7777 } } });
      assert.equal(v.value, false);
      assert.equal(v.disabled, true);
    });

    it("read_only from meta makes a regular switch locked", () => {
      const field = { path: "web.enabled", kind: "toggle", title: "Node web page" };
      const v = fieldView(field, ctx({ meta: meta_({ read_only: ["web.enabled"] }) }));
      assert.equal(v.readOnly, true);
      assert.equal(v.disabled, true);
      assert.equal(v.value, true);
    });
  });

  describe("read-only, non-boolean value", () => {
    it("port — a value in a frame, not boolean", () => {
      const v = viewOf("web.port", {});
      assert.equal(v.readOnly, true);
      assert.equal(v.editable, false);
      assert.equal(String(v.value), "7777");
    });
  });

  describe("switch without restrictions", () => {
    it("is editable: editable, not locked, not read-only", () => {
      const field = { path: "web.enabled", kind: "toggle", title: "Node web page" };
      const v = fieldView(field, ctx({}));
      assert.equal(v.editable, true);
      assert.equal(v.disabled, false);
      assert.equal(v.readOnly, false);
    });
  });

  describe("\"after restart\" mark", () => {
    it("restart_fields — present", () => {
      assert.equal(viewOf("socks.port", { meta: meta_({ restart_fields: ["socks.port"] }) }).restartBadge, true);
    });

    it("restart_pending — present", () => {
      assert.equal(viewOf("socks.port", { meta: meta_({ restart_pending: ["socks.port"] }) }).restartBadge, true);
    });

    it("both lists — one mark (boolean), not two", () => {
      const meta = meta_({ restart_fields: ["socks.port"], restart_pending: ["socks.port"] });
      assert.equal(viewOf("socks.port", { meta }).restartBadge, true);
    });

    it("in neither — absent", () => {
      assert.equal(viewOf("socks.port", {}).restartBadge, false);
    });
  });

  describe("field value", () => {
    it("without a draft — the config value, as a string", () => {
      assert.equal(viewOf("socks.port", {}).value, "1080");
    });

    it("the draft wins over the config and stays raw", () => {
      assert.equal(viewOf("socks.port", { draft: { "socks.port": "1081" } }).value, "1081");
    });

    it("an empty string in the draft — an empty value, not the config value", () => {
      assert.equal(viewOf("socks.port", { draft: { "socks.port": "" } }).value, "");
    });
  });

  describe("field error", () => {
    it("the text is taken from errors by path; otherwise empty", () => {
      assert.equal(viewOf("socks.port", { errors: { "socks.port": "in use" } }).error, "in use");
      assert.ok(!viewOf("socks.port", {}).error);
    });
  });

  describe("notes", () => {
    it("read-only without a flag — \"Can only be changed in the settings file\"", () => {
      assert.equal(viewOf("web.port", {}).readonlyNote, "Can only be changed in the settings file");
    });

    it("web.enabled — its own reason about access to the page itself", () => {
      assert.equal(
        viewOf("web.enabled", {}).readonlyNote,
        "Can only be turned off in the settings file — otherwise the page would lock itself out",
      );
    });

    it("a field overridden by a flag carries cliNote from cliNoteText", () => {
      const meta = meta_({ cli_overrides: ["socks.port"] });
      const v = viewOf("socks.port", { meta, state: { socks: { listen: "127.0.0.1:2000" } } });
      assert.equal(v.cliNote, cliNoteText("socks.port", false, { socks: { listen: "127.0.0.1:2000" } }, ctx({}).config));
      assert.match(v.cliNote, /currently in effect: 2000/);
    });

    it("without a flag cliNote is empty", () => {
      assert.ok(!viewOf("socks.port", {}).cliNote);
    });
  });

  describe("disabledBy lock", () => {
    it("autostart unavailable — the field is locked, the reason is from the config", () => {
      const config = { ...ctx({}).config, autostart: { available: false, reason: "no systemd", enabled: false } };
      const v = viewOf("autostart.enabled", { config });
      assert.equal(v.disabled, true);
      assert.equal(v.disabledReason, "no systemd");
    });

    it("autostart available — not locked", () => {
      const config = { ...ctx({}).config, autostart: { available: true, reason: "", enabled: false } };
      assert.equal(viewOf("autostart.enabled", { config }).disabled, false);
    });
  });
});

describe("cliNoteText", () => {
  const config = { socks: { port: 1080 } };

  it("socks.listen: null — without a port number, differently for editing and for read-only", () => {
    const state = { socks: { listen: null } };
    assert.equal(
      cliNoteText("socks.port", false, state, config),
      "set at startup — your edit will be saved to the file and take effect when you start Mesh Town without this command-line option",
    );
    assert.equal(cliNoteText("socks.port", true, state, config), "set at startup — can't be changed here");
  });

  it("listen 127.0.0.1:2000 — the file and the effective port", () => {
    const state = { socks: { listen: "127.0.0.1:2000" } };
    assert.match(
      cliNoteText("socks.port", false, state, config),
      /^in file: 1080 · currently in effect: 2000 — set at startup, your edit will be saved/,
    );
    assert.match(
      cliNoteText("socks.port", false, state, config),
      /take effect when you start Mesh Town without this command-line option$/,
    );
    assert.equal(
      cliNoteText("socks.port", true, state, config),
      "in file: 1080 · currently in effect: 2000 — set at startup",
    );
  });

  it("web.port is taken from state.node.web_addr", () => {
    const text = cliNoteText("web.port", true, { node: { web_addr: "127.0.0.1:7788" } }, { web: { port: 7777 } });
    assert.equal(text, "in file: 7777 · currently in effect: 7788 — set at startup");
  });

  it("state null — does not crash, the text has no port", () => {
    assert.equal(cliNoteText("socks.port", true, null, config), "set at startup — can't be changed here");
  });

  it("the text changes when socks.listen appears in the snapshot", () => {
    const before = cliNoteText("socks.port", false, { socks: { listen: null } }, config);
    const after = cliNoteText("socks.port", false, { socks: { listen: "127.0.0.1:2000" } }, config);
    assert.notEqual(before, after);
    assert.match(after, /currently in effect: 2000/);
  });
});

describe("savedMessageText", () => {
  const meta = meta_({ restart_fields: ["socks.port"], restart_pending: ["socks.enabled"] });

  it("a field from restart_fields — \"Takes effect after Mesh Town restarts\"", () => {
    assert.equal(savedMessageText(meta, ["socks.port"]), "Saved. Takes effect after Mesh Town restarts.");
  });

  it("a field from restart_pending — the same", () => {
    assert.equal(savedMessageText(meta, ["socks.enabled"]), "Saved. Takes effect after Mesh Town restarts.");
  });

  it("another field or an empty list — \"Saved\"", () => {
    assert.equal(savedMessageText(meta, ["checks.enabled"]), "Saved");
    assert.equal(savedMessageText(meta, []), "Saved");
  });
});

describe("fileBanner", () => {
  describe("file cannot be read", () => {
    it("invalid — broken with the file's message", () => {
      const b = fileBanner({ state: "invalid", message: "broken json" }, false);
      assert.equal(b.broken.message, "broken json");
    });

    it("unreadable without a message — the fallback text", () => {
      const b = fileBanner({ state: "unreadable" }, false);
      assert.equal(b.broken.message, "The settings file is damaged");
    });
  });

  describe("field errors", () => {
    const configFile = { state: "ok", field_errors: [{ path: "socks.port", message: "not a number" }] };

    it("are shown until hidden", () => {
      const b = fileBanner(configFile, false);
      assert.equal(b.broken, null);
      assert.deepEqual(b.fieldErrors, configFile.field_errors);
    });

    it("after a successful save (hideFieldErrors) — nothing to show, null", () => {
      assert.equal(fileBanner(configFile, true), null);
    });
  });

  describe("nothing to show", () => {
    it("no file, a clean file — null", () => {
      assert.equal(fileBanner(null, false), null);
      assert.equal(fileBanner({ state: "ok", field_errors: [] }, false), null);
    });
  });
});

describe("saveBarView", () => {
  describe("bar class", () => {
    it("edits only — \"save-bar dirty\"", () => {
      assert.equal(bar({ dirty: true }).cls, "save-bar dirty");
    });

    it("no edits, a restart is pending — sticky, even after the restart request", () => {
      assert.match(bar({ pending: ["socks.port"], requested: true }).cls, /saved-sticky/);
    });

    it("edits and pending — dirty, without saved-sticky", () => {
      const cls = bar({ dirty: true, pending: ["socks.port"] }).cls;
      assert.match(cls, /dirty/);
      assert.doesNotMatch(cls, /saved-sticky/);
    });

    it("nothing — \"save-bar\"", () => {
      assert.equal(bar({}).cls, "save-bar");
    });
  });

  describe("restart button", () => {
    it("pending exists, no request was made — offered", () => {
      assert.equal(bar({ pending: ["socks.port"] }).restartOffered, true);
    });

    it("after the request — not offered, the bar stays", () => {
      const v = bar({ pending: ["socks.port"], requested: true });
      assert.equal(v.restartOffered, false);
      assert.equal(v.visible, true);
    });

    it("no pending — not offered", () => {
      assert.equal(bar({}).restartOffered, false);
    });
  });

  describe("visibility", () => {
    it("hidden without edits, without pending and with a writable file", () => {
      assert.equal(bar({}).visible, false);
      assert.ok(!bar({}).dirtyHint);
    });

    it("visible with edits, with an \"unsaved\" hint", () => {
      const v = bar({ dirty: true });
      assert.equal(v.visible, true);
      assert.ok(v.dirtyHint, "the hint about unsaved edits is not shown");
    });

    it("visible with a pending restart", () => {
      assert.equal(bar({ pending: ["socks.port"] }).visible, true);
    });
  });

  describe("file is not writable", () => {
    it("the bar is always visible, \"Save\" is locked, the reason is the file's message", () => {
      const v = bar({ configFile: { writable: false, message: "read-only directory" } });
      assert.equal(v.visible, true);
      assert.equal(v.saveDisabled, true);
      assert.equal(v.writableReason, "Read-only directory");
    });

    it("without a message — a fallback reason, not empty", () => {
      const v = bar({ configFile: { writable: false } });
      assert.equal(v.writableReason, "Can't save: the folder with the settings file is read-only");
    });

    it("the file is writable — \"Save\" is available", () => {
      assert.equal(bar({ configFile: { writable: true } }).saveDisabled, false);
    });
  });

  describe("saving in progress", () => {
    it("\"Save\" is locked", () => {
      assert.equal(bar({ busy: true }).saveDisabled, true);
    });
  });

  describe("message text", () => {
    it("RESTART_STARTED names the program, without the word \"Node\"", () => {
      assert.equal(RESTART_STARTED, NET_NAME + " is restarting. The settings will take effect in a few seconds.");
    });

    it("\"Mesh Town is restarting\" with an empty restart_pending — is not shown", () => {
      assert.equal(bar({ pending: [], text: RESTART_STARTED }).messageShown, "");
    });

    it("\"Mesh Town is restarting\" with a non-empty restart_pending — is shown", () => {
      assert.equal(bar({ pending: ["socks.port"], requested: true, text: RESTART_STARTED }).messageShown, RESTART_STARTED);
    });

    it("other text with an empty restart_pending — is shown as is", () => {
      assert.equal(bar({ pending: [], text: "Saved" }).messageShown, "Saved");
    });

    it("no meta — like an empty restart_pending", () => {
      const v = saveBarView({ dirty: false, meta: null, configFile: null, requested: false, busy: false, message: msg(RESTART_STARTED) });
      assert.equal(v.messageShown, "");
    });
  });
});

function meta_(over = {}) {
  return { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [], ...over };
}

function ctx({ config, meta, state = null, draft = {}, errors = {} }) {
  return {
    config: config || {
      web: { enabled: true, port: 7777 },
      socks: { enabled: true, port: 1080 },
      autostart: { available: true, reason: "", enabled: false },
      full_tunnel: { available: true, reason: "", enabled: false },
    },
    meta: meta || meta_(),
    state,
    draft,
    errors,
  };
}

function viewOf(path, over) {
  const field = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].find((f) => f.path === path);
  assert.ok(field, "no field " + path);
  return fieldView(field, ctx(over));
}

// `message` in `saveBarView` is the `form.message` object `{text, cls}` (as in `createSettingsForm`).
function msg(text) {
  return { text, cls: "saved" };
}

function bar({ dirty = false, pending = [], requested = false, busy = false, configFile = null, text = "" }) {
  return saveBarView({
    dirty,
    meta: meta_({ restart_pending: pending }),
    configFile,
    restartTargets: pending.length > 0 ? ["main"] : [],
    requested,
    busy,
    message: msg(text),
  });
}
