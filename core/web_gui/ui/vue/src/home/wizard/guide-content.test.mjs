/* The content of the guide screen and the browser tiles (`guide-content.ts`): the steps by scope, system and
 * browser, the tiles order by system, the default browser and the browser used by the manual block. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ANDROID_INTRO, ANDROID_STEPS, FIREFOX_STEPS, IOS_NO_APP, MAC_STEPS } from "../../common/pac-texts.ts";
import { PAC_INTRO_ROUTER } from "../../lan/nets-texts.ts";
import { SETTINGS_SOCKS_LOCALNET } from "../../settings/settings-texts.ts";
import {
  ANDROID_APP_SMALL, ANDROID_WIFI_STEPS, DEVICE_LABELS, IOS_ROUTER_LINK, IOS_VIA_COMPUTER, IOS_WIFI_STEPS, ROUTER_GUIDE_TITLE, SITE_DEVICES_URL, WIFI_ONLY_SMALL,
} from "./device-texts.ts";
import { UPDATE_ANDROID_DOWNLOAD_URL } from "../update-texts.ts";
import { pacBlock } from "../pac-setup.ts";
import { browserTiles, defaultBrowser, guideFor, manualBrowser } from "./guide-content.ts";
import { ALL_APPS_BROWSER_STEPS, ALL_APPS_BROWSERS_TITLE, ALL_APPS_LINUX_SMALL, WIZ_APP_ITEM, WIZ_ONE_BROWSER, WIZ_ONE_BROWSER_OTHER } from "./wizard-texts.ts";

const BROWSERS = ["chrome", "edge", "safari", "firefox", "other"];
const guide = (scope, os, browser, lan = "none") => guideFor(scope, os, browser, lan);
const ids = (tiles) => tiles.map(([id]) => id);

describe("guideFor", () => {
  describe("whole system on a computer", () => {
    for (const os of ["mac", "windows", "linux"]) {
      it(`given system on ${os}, then it copies the address and shows the system steps and more options`, () => {
        const g = guide("system", os, "chrome");
        assert.equal(g.copy, true);
        assert.deepEqual(g.steps.map((s) => s.text), pacBlock(os).steps);
        assert.equal(g.more, true);
        assert.equal(g.toSystem, false);
      });
    }

    for (const browser of BROWSERS) {
      it(`given system on mac with ${browser}, then the instruction is the same`, () => {
        assert.deepEqual(guide("system", "mac", browser), guide("system", "mac", "chrome"));
      });
    }

    for (const [os, label] of [["mac", "macOS"], ["windows", "Windows"], ["linux", "Linux"]]) {
      it(`given system on ${os}, then the title names the system and the browser block is constant`, () => {
        const g = guide("system", os, "firefox");
        assert.equal(g.title, `Set up all apps on ${label}`);
        assert.equal(g.browserTitle, ALL_APPS_BROWSERS_TITLE);
        assert.deepEqual(g.browserSteps, ALL_APPS_BROWSER_STEPS);
      });
    }

    it("given system on Linux, then the small Chromium line is present", () => {
      assert.deepEqual(guide("system", "linux", "chrome").browserSmall, [ALL_APPS_LINUX_SMALL]);
    });

    for (const os of ["mac", "windows"]) {
      it(`given system on ${os}, then the small Chromium line is absent`, () => {
        assert.deepEqual(guide("system", os, "chrome").browserSmall, []);
      });
    }
  });

  describe("only in one browser", () => {
    it("given Firefox, then it copies the address and shows the Firefox steps and more options", () => {
      const g = guide("browser", "mac", "firefox");
      assert.equal(g.copy, true);
      assert.deepEqual(g.steps.map((s) => s.text), FIREFOX_STEPS);
      assert.equal(g.more, true);
    });

    for (const [browser, name] of [["chrome", "Chrome"], ["edge", "Edge"], ["safari", "Safari"]]) {
      it(`given ${name}, then it explains the system setting and offers to set up all apps`, () => {
        const g = guide("browser", "mac", browser);
        assert.equal(g.intro, WIZ_ONE_BROWSER(name));
        assert.equal(g.toSystem, true);
        assert.equal(g.copy, false);
        assert.equal(g.more, false);
      });
    }

    it("given another browser, then the address is shown with the intro and the switch to all apps", () => {
      const g = guide("browser", "mac", "other");
      assert.equal(g.intro, WIZ_ONE_BROWSER_OTHER);
      assert.equal(g.toSystem, true);
      assert.equal(g.copy, true);
      assert.equal(g.more, false);
    });
  });

  describe("on a phone", () => {
    it("given Android, then the first step links to the app download", () => {
      const g = guide("system", "android", "chrome");
      assert.equal(g.steps[0].href, UPDATE_ANDROID_DOWNLOAD_URL);
      assert.equal(g.steps[0].text, WIZ_APP_ITEM);
    });

    it("given Android, then the other steps follow the app steps", () => {
      // The steps after the download item are expected to be ANDROID_STEPS in order.
      const g = guide("system", "android", "chrome");
      assert.deepEqual(g.steps.slice(1).map((s) => s.text), ANDROID_STEPS);
    });

    it("given Android, then there is no copying and no more options", () => {
      const g = guide("system", "android", "chrome");
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
      assert.equal(g.intro, ANDROID_INTRO);
    });

    it("given Android and the browser scope, then the guide is the same", () => {
      const g = guide("browser", "android", "chrome");
      assert.equal(g.steps[0].href, UPDATE_ANDROID_DOWNLOAD_URL);
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
    });

  });

  describe("on a router", () => {
    it("given a Mac, then it shows the router intro, the system steps and the browser block", () => {
      const g = guide("system", "mac", "chrome", "router");
      assert.equal(g.title, ROUTER_GUIDE_TITLE(DEVICE_LABELS.mac));
      assert.equal(g.title, "Connect a Mac to the router");
      assert.equal(g.copy, true);
      assert.equal(g.intro, PAC_INTRO_ROUTER);
      assert.deepEqual(g.steps.map((s) => s.text), MAC_STEPS);
      assert.equal(g.browserTitle, ALL_APPS_BROWSERS_TITLE);
      assert.deepEqual(g.browserSteps, ALL_APPS_BROWSER_STEPS);
    });

    it("given an iPhone, then it shows the Wi-Fi steps and no browser block", () => {
      const g = guide("system", "ios", "chrome", "router");
      assert.equal(g.title, "Connect an iPhone or iPad to the router");
      assert.equal(g.copy, true);
      assert.deepEqual(g.steps.map((s) => s.text), IOS_WIFI_STEPS);
      assert.deepEqual(g.small, [WIFI_ONLY_SMALL]);
      assert.deepEqual(g.browserSteps, []);
      assert.equal(g.browserTitle, "");
    });

    it("given Android, then it shows the Wi-Fi steps and the app hint", () => {
      const g = guide("system", "android", "chrome", "router");
      assert.deepEqual(g.steps.map((s) => s.text), ANDROID_WIFI_STEPS);
      assert.deepEqual(g.small, [WIFI_ONLY_SMALL, ANDROID_APP_SMALL]);
      assert.equal(g.steps[0].href, undefined);
    });

    for (const os of ["mac", "windows", "linux", "ios", "android"]) {
      it(`given ${os}, then the scope and the browser do not change the instruction`, () => {
        const base = guide("system", os, "chrome", "router");
        assert.deepEqual(guide("browser", os, "firefox", "router"), base);
        assert.deepEqual(guide("browser", os, "other", "router"), base);
      });
    }
  });

  describe("an iPhone off the router", () => {
    it("given the node shares its proxy, then it copies the address and shows the Wi-Fi steps", () => {
      const g = guide("system", "ios", "chrome", "shared");
      assert.equal(g.copy, true);
      assert.equal(g.intro, IOS_VIA_COMPUTER);
      assert.deepEqual(g.steps.map((s) => s.text), IOS_WIFI_STEPS);
      assert.deepEqual(g.small, [WIFI_ONLY_SMALL]);
    });

    it("given the proxy is not shared, then it offers the router guide and the setting", () => {
      const g = guide("system", "ios", "chrome", "none");
      assert.equal(g.intro, IOS_NO_APP);
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
      assert.equal(g.steps[0].href, SITE_DEVICES_URL);
      assert.equal(g.steps[0].link, IOS_ROUTER_LINK);
      assert.ok(g.steps[1].text.includes(SETTINGS_SOCKS_LOCALNET));
    });
  });
});

describe("browserTiles", () => {
  it("given macOS, then the order is Safari, Chrome, Firefox, Edge, Other", () => {
    assert.deepEqual(ids(browserTiles("mac")), ["safari", "chrome", "firefox", "edge", "other"]);
  });

  it("given Windows, then the order is Chrome, Edge, Firefox, Other", () => {
    assert.deepEqual(ids(browserTiles("windows")), ["chrome", "edge", "firefox", "other"]);
  });

  it("given Linux, then the order is Firefox, Chrome, Other", () => {
    assert.deepEqual(ids(browserTiles("linux")), ["firefox", "chrome", "other"]);
  });

  it("given a phone system, then there are no tiles", () => {
    assert.deepEqual(browserTiles("android"), []);
    assert.deepEqual(browserTiles("ios"), []);
  });
});

describe("defaultBrowser", () => {
  it("given the detected browser is among the tiles, then it is the default", () => {
    assert.equal(defaultBrowser("windows", "edge"), "edge");
  });

  it("given the detected browser is not among the tiles, then the default is other", () => {
    assert.equal(defaultBrowser("linux", "safari"), "other");
  });
});

describe("manualBrowser", () => {
  it("given other, then the manual block uses chrome", () => {
    assert.equal(manualBrowser("other"), "chrome");
  });

  it("given firefox, then the manual block uses firefox", () => {
    assert.equal(manualBrowser("firefox"), "firefox");
  });
});
