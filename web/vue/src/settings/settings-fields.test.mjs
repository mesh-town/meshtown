/* The "Settings" fields and value parsing: the field set, `parseRaw`, `controlValue`, `collectChanges`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { reactive } from "vue";
import {
  MAIN_FIELDS,
  EXPERT_FIELDS,
  PAGE_FIELDS,
  TRANSPORT_MODE_OPTIONS,
  fid,
  getPath,
  parseRaw,
  controlValue,
  selectOptions,
  collectChanges,
  fieldErrorLine,
} from "./settings-fields.ts";

describe("fields", () => {
  describe("\"Main\"", () => {
    it("six paths in the classic order, proxy access — after the port", () => {
      assert.deepEqual(MAIN_FIELDS.map((f) => f.path), [
        "autostart.enabled",
        "socks.enabled",
        "socks.port",
        "socks.access",
        "checks.enabled",
        "checks.interval_sec",
      ]);
    });

    it("socks.access: a choice of two values, an explanation about the proxy without a password", () => {
      const f = MAIN_FIELDS.find((x) => x.path === "socks.access");
      assert.equal(f.kind, "select");
      assert.equal(f.title, "Who can use the proxy");
      assert.deepEqual(f.options.map((o) => o[0]), ["localhost", "localnet"]);
      assert.deepEqual(f.options.map((o) => o[1]), ["This computer only", "Local network devices (TV, phone)"]);
      assert.match(f.desc, /proxy has no password/);
      assert.match(f.desc, /after the node restarts/);
      assert.deepEqual(collectChanges({ "socks.access": "localnet" }, MAIN_FIELDS), { "socks.access": "localnet" });
    });

    it("check titles say \"sites\", not \"domains\"", () => {
      assert.equal(titleOf("checks.enabled"), "Check sites from the list");
      assert.equal(titleOf("checks.interval_sec"), "How often to check sites");
    });

    it("there is no relay mode choice (transport.mode is hidden)", () => {
      const all = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].map((f) => f.path);
      assert.ok(!all.includes("transport.mode"));
      assert.ok(!MAIN_FIELDS.some((f) => /relay/.test(f.title + (f.desc || ""))));
    });
  });

  describe("\"For specialists\"", () => {
    it("does not overlap with \"Main\", carries the remaining fields from the classic page", () => {
      const main = new Set(MAIN_FIELDS.map((f) => f.path));
      const expert = EXPERT_FIELDS.map((f) => f.path);
      for (const p of expert) assert.equal(main.has(p), false, p + " in both lists");
      for (const p of ["checks.doh_server", "checks.provider_dns",
        "probe.interval_sec", "log.ring_lines", "full_tunnel.enabled"]) {
        assert.ok(expert.includes(p), p + " not in \"For specialists\"");
      }
    });
  });

  it("does not carry the retired transport fields", () => {
    const retired = ["udp_port", "congestion_control", "mtu_upper_bound", "stream_receive_window",
      "receive_window", "max_concurrent_bidi_streams", "loss_packet_threshold", "loss_time_threshold"]
      .map((k) => "transport." + k);
    for (const f of [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS]) {
      assert.equal(retired.includes(f.path), false, f.path + " was removed from the config in 0.1.38");
    }
  });

  describe("\"Page\"", () => {
    it("web.enabled and web.port are read-only, web.access is a choice", () => {
      assert.deepEqual(PAGE_FIELDS.map((f) => [f.path, f.kind]), [["web.enabled", "readonly"], ["web.port", "readonly"], ["web.access", "select"]]);
    });

    it("web.access: two values, English labels, an explanation about access to the settings, not to the network", () => {
      const f = PAGE_FIELDS.find((x) => x.path === "web.access");
      assert.equal(f.dataType, "str");
      assert.deepEqual(f.options.map((o) => o[0]), ["localhost", "localnet"]);
      assert.ok(f.options.every((o) => /[a-z]/i.test(o[1])));
      assert.match(f.desc, /this only concerns access to the node settings, not to the network or traffic/i);
    });

    it("web.access goes into PUT /api/config via the shared collectChanges path", () => {
      assert.deepEqual(collectChanges({ "web.access": "localhost" }, PAGE_FIELDS), { "web.access": "localhost" });
    });
  });
});

describe("fid", () => {
  it("dots become hyphens, prefix field-", () => {
    assert.equal(fid("checks.provider_dns"), "field-checks-provider_dns");
    assert.equal(fid("web.enabled"), "field-web-enabled");
  });
});

describe("getPath", () => {
  it("value by a dotted path; no node — undefined", () => {
    const o = { a: { b: 0, c: null } };
    assert.equal(getPath(o, "a.b"), 0);
    assert.equal(getPath(o, "a.x"), undefined);
    assert.equal(getPath(o, "a.c.d"), undefined);
    assert.equal(getPath(o, "z.y"), undefined);
  });
});

describe("parseRaw", () => {
  describe("int", () => {
    it("empty → null, \"12\" → 12, \"12abc\" → 12, \"abc\" → the raw string", () => {
      assert.equal(parseRaw("", "int"), null);
      assert.equal(parseRaw("12", "int"), 12);
      assert.equal(parseRaw("12abc", "int"), 12);
      assert.equal(parseRaw("abc", "int"), "abc");
    });

    it("int_or_null behaves the same way", () => {
      assert.equal(parseRaw("", "int_or_null"), null);
      assert.equal(parseRaw("12", "int_or_null"), 12);
      assert.equal(parseRaw("abc", "int_or_null"), "abc");
    });
  });

  describe("str_or_null", () => {
    it("spaces → null; non-empty stays as is", () => {
      assert.equal(parseRaw(" ", "str_or_null"), null);
      assert.equal(parseRaw("8.8.8.8:53", "str_or_null"), "8.8.8.8:53");
    });
  });

  describe("lines", () => {
    it("strings are trimmed, empty ones are dropped", () => {
      assert.deepEqual(parseRaw("a\n \n b", "lines"), ["a", "b"]);
      assert.deepEqual(parseRaw("", "lines"), []);
    });
  });

  describe("bool and str", () => {
    it("bool — as is; str — a string without parsing", () => {
      assert.equal(parseRaw(true, "bool"), true);
      assert.equal(parseRaw(false, "bool"), false);
      assert.equal(parseRaw(" x ", "str"), " x ");
    });
  });
});

describe("controlValue", () => {
  it("null and undefined → empty string", () => {
    assert.equal(controlValue(null, "text"), "");
    assert.equal(controlValue(undefined, "text"), "");
  });

  it("lines: an array → strings joined by newlines", () => {
    assert.equal(controlValue(["a", "b"], "lines"), "a\nb");
  });

  it("toggle: 1 → true, 0 → false", () => {
    assert.equal(controlValue(1, "toggle"), true);
    assert.equal(controlValue(0, "toggle"), false);
  });

  it("a number for text and select → string", () => {
    assert.equal(controlValue(1080, "text"), "1080");
    assert.equal(controlValue(300, "select"), "300");
  });
});

describe("selectOptions", () => {
  const field = { options: [[300, "every 5 minutes"], [900, "every 15 minutes"]] };

  it("the current value is in the list — options without additions", () => {
    assert.equal(selectOptions(field, 300).length, 2);
  });

  it("the value is not in the list — it is appended as an option with the same value", () => {
    const opts = selectOptions(field, 42);
    assert.equal(opts.length, 3);
    assert.deepEqual(opts[2], [42, "42 — set in the file"]);
  });
});

const ALL_FIELDS = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS];

const WITH_LIST = [...ALL_FIELDS, { path: "z.list", kind: "lines", dataType: "lines", title: "z", desc: "" }];

describe("collectChanges", () => {
  describe("parsing by dataType", () => {
    it("every draft path is parsed by the field type", () => {
      const out = collectChanges(
        { "socks.port": "1081", "checks.provider_dns": " ", "socks.enabled": false, "z.list": "a\n b" },
        WITH_LIST,
      );
      assert.deepEqual(plain(out), {
        "socks.port": 1081,
        "checks.provider_dns": null,
        "socks.enabled": false,
        "z.list": ["a", "b"],
      });
    });

    it("an empty draft — an empty object", () => {
      assert.deepEqual(plain(collectChanges({}, ALL_FIELDS)), {});
    });
  });

  describe("reactive draft", () => {
    it("structuredClone of the result does not throw, lines is a plain array", () => {
      const draft = reactive({ "z.list": "a\nb" });
      const changes = collectChanges(draft, WITH_LIST);
      let copy;
      assert.doesNotThrow(() => (copy = structuredClone(changes)), "a Vue proxy leaked into the result");
      assert.deepEqual(copy["z.list"], ["a", "b"]);
    });

    it("returns a new object, not the draft itself", () => {
      const draft = reactive({ "socks.port": "1" });
      assert.notEqual(collectChanges(draft, ALL_FIELDS), draft);
    });
  });
});

function titleOf(path) {
  return ALL_FIELDS.find((f) => f.path === path).title;
}

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

describe("fieldErrorLine", () => {
  it("a field from the list — title and path", () =>
    assert.equal(
      fieldErrorLine({ path: "socks.port", message: "in use" }),
      "Proxy port (SOCKS5 and HTTP) (socks.port): in use",
    ));
  it("the field is not in the lists — path and message", () =>
    assert.equal(fieldErrorLine({ path: "node.name", message: "empty" }), "node.name: empty"));
});

describe("network selection texts", () => {
  const webAccess = () => PAGE_FIELDS.find((f) => f.path === "web.access");

  it("page access options without the word localhost", () => {
    const texts = webAccess().options.map((o) => o[1]);
    assert.deepEqual([...texts].sort(), ["From this computer only", "From this computer and the local network"].sort());
  });

  it("the option values are unchanged", () => {
    assert.deepEqual(webAccess().options.map((o) => o[0]).sort(), ["localhost", "localnet"]);
  });

  it("neither the labels nor the field descriptions in \"Settings\" have \"(localhost)\"", () => {
    const fields = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS];
    const texts = fields.flatMap((f) => [f.title, f.desc, ...(f.options || []).map((o) => o[1])]).filter(Boolean);
    assert.ok(texts.every((t) => !t.includes("(localhost)")), "the word localhost in parentheses is left in the text");
  });

  it("the socks.access description in the field itself is unchanged (about \"your network\" and guest Wi-Fi) — no networks block", () => {
    const f = MAIN_FIELDS.find((x) => x.path === "socks.access");
    assert.equal(
      f.desc,
      "In local network mode the proxy has no password: any device on your network (and guest Wi-Fi, if it isn't separated) can use it. Takes effect after the node restarts.",
    );
  });
});
