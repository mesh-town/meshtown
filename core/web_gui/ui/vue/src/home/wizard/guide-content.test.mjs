/* The content of the guide screen and the browser tiles (`guide-content.ts`): the steps by scope, system and
 * browser, the tiles order by system, the default browser and the browser used by the manual block. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ANDROID_INTRO, ANDROID_STEPS, FIREFOX_STEPS, IOS_NO_APP } from "../../common/pac-texts.ts";
import { UPDATE_ANDROID_DOWNLOAD_URL } from "../update-texts.ts";
import { pacBlock } from "../pac-setup.ts";
import { browserTiles, defaultBrowser, guideFor, manualBrowser } from "./guide-content.ts";
import { ALL_APPS_CHROME_LINUX, ALL_APPS_STEPS, WIZ_APP_ITEM, WIZ_ONE_BROWSER, WIZ_ONE_BROWSER_OTHER } from "./wizard-texts.ts";

const BROWSERS = ["chrome", "edge", "safari", "firefox", "other"];
const ids = (tiles) => tiles.map(([id]) => id);

describe("guideFor", () => {
  describe("whole system on a computer", () => {
    for (const os of ["mac", "windows", "linux"]) {
      it(`given system on ${os}, then it copies the address and shows the system steps and more options`, () => {
        const g = guideFor("system", os, "chrome");
        assert.equal(g.copy, true);
        assert.deepEqual(g.steps.map((s) => s.text), pacBlock(os).steps);
        assert.equal(g.more, true);
        assert.equal(g.toSystem, false);
      });
    }

    for (const browser of BROWSERS) {
      it(`given system on mac with ${browser}, then the browser steps are taken verbatim`, () => {
        assert.deepEqual(guideFor("system", "mac", browser).browserSteps, ALL_APPS_STEPS[browser]);
      });
    }

    it("given system on Linux with Chrome, then the small Chrome line is present", () => {
      // The line is expected in `browserSmall` (the small lines of the browser block).
      assert.ok(guideFor("system", "linux", "chrome").browserSmall.includes(ALL_APPS_CHROME_LINUX));
    });

    it("given system on macOS with Chrome, then the small Chrome line is absent", () => {
      assert.ok(!guideFor("system", "mac", "chrome").browserSmall.includes(ALL_APPS_CHROME_LINUX));
    });
  });

  describe("only in one browser", () => {
    it("given Firefox, then it copies the address and shows the Firefox steps and more options", () => {
      const g = guideFor("browser", "mac", "firefox");
      assert.equal(g.copy, true);
      assert.deepEqual(g.steps.map((s) => s.text), FIREFOX_STEPS);
      assert.equal(g.more, true);
    });

    for (const [browser, name] of [["chrome", "Chrome"], ["edge", "Edge"], ["safari", "Safari"]]) {
      it(`given ${name}, then it explains the system setting and offers to set up all apps`, () => {
        const g = guideFor("browser", "mac", browser);
        assert.equal(g.intro, WIZ_ONE_BROWSER(name));
        assert.equal(g.toSystem, true);
        assert.equal(g.copy, false);
        assert.equal(g.more, false);
      });
    }

    it("given another browser, then the intro speaks about this browser", () => {
      const g = guideFor("browser", "mac", "other");
      assert.equal(g.intro, WIZ_ONE_BROWSER_OTHER);
      assert.equal(g.toSystem, true);
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
    });
  });

  describe("on a phone", () => {
    it("given Android, then the first step links to the app download", () => {
      const g = guideFor("system", "android", "chrome");
      assert.equal(g.steps[0].href, UPDATE_ANDROID_DOWNLOAD_URL);
      assert.equal(g.steps[0].text, WIZ_APP_ITEM);
    });

    it("given Android, then the other steps follow the app steps", () => {
      // The steps after the download item are expected to be ANDROID_STEPS in order.
      const g = guideFor("system", "android", "chrome");
      assert.deepEqual(g.steps.slice(1).map((s) => s.text), ANDROID_STEPS);
    });

    it("given Android, then there is no copying and no more options", () => {
      const g = guideFor("system", "android", "chrome");
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
      assert.equal(g.intro, ANDROID_INTRO);
    });

    it("given Android and the browser scope, then the guide is the same", () => {
      const g = guideFor("browser", "android", "chrome");
      assert.equal(g.steps[0].href, UPDATE_ANDROID_DOWNLOAD_URL);
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
    });

    it("given iOS, then the intro says there is no app and there are no steps", () => {
      const g = guideFor("system", "ios", "chrome");
      assert.equal(g.intro, IOS_NO_APP);
      assert.deepEqual(g.steps, []);
      assert.equal(g.copy, false);
      assert.equal(g.more, false);
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
