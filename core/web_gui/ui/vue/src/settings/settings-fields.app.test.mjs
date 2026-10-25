/* `selectOptions` inside the Android app (the user agent carries `MeshTownApp/`): the option labels say "phone". */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { MAIN_FIELDS, EXPERT_FIELDS, PAGE_FIELDS, selectOptions } from "./settings-fields.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const field = (path) => [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].find((f) => f.path === path);
const labels = (path, value) => selectOptions(field(path), value).map((pair) => pair[1]);

describe("selectOptions", () => {
  it("in the app: access options name this phone", () => {
    vi.stubGlobal("navigator", { userAgent: APP_UA });
    assert.deepEqual(labels("web.access", "localhost"), ["From this phone only", "From this phone and the local network"]);
    assert.deepEqual(labels("socks.access", "localhost"), ["This phone only", "Local network devices (TV, phone)"]);
  });

  it("in the app: an unknown value is appended after the translated options", () => {
    vi.stubGlobal("navigator", { userAgent: APP_UA });
    const list = labels("web.access", "elsewhere");
    assert.equal(list.length, 3);
    assert.equal(list[0], "From this phone only");
  });

  it("on a desktop: the labels are as before", () => {
    vi.stubGlobal("navigator", { userAgent: DESKTOP_UA });
    assert.deepEqual(labels("web.access", "localhost"), ["From this computer only", "From this computer and the local network"]);
    assert.deepEqual(labels("socks.access", "localnet"), ["This computer only", "Local network devices (TV, phone)"]);
  });
});
