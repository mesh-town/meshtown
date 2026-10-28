/* Texts of the step-by-step setup (`wizard-texts.ts`): verbatim values — the wizard screens are checked against
 * them. The product name comes from `NET_NAME`, never as a literal inside the text sources. */

import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "../../common/words.ts";
import { SCOPE_QUESTION } from "../../common/pac-texts.ts";
import { HOME_WIZARD_DONE, HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_STEP_EXIT } from "../home-card-texts.ts";
import * as texts from "./wizard-texts.ts";

describe("progress and navigation", () => {
  describe("progress line", () => {
    it("WIZ_PROGRESS(2, 6) is \"Step 2 of 6\"", () => {
      assert.equal(texts.WIZ_PROGRESS(2, 6), "Step 2 of 6");
    });

    it("WIZ_PROGRESS(1, 3) is \"Step 1 of 3\"", () => {
      assert.equal(texts.WIZ_PROGRESS(1, 3), "Step 1 of 3");
    });
  });

  describe("buttons", () => {
    it("Back, Next, Skip this step, Finish", () => {
      assert.equal(texts.WIZ_BACK, "Back");
      assert.equal(texts.WIZ_NEXT, "Next");
      assert.equal(texts.WIZ_SKIP, "Skip this step");
      assert.equal(texts.WIZ_FINISH, "Finish");
    });

    it("the last screen of the app path ends with \"Done\", not \"Finish\"", () => {
      assert.equal(HOME_WIZARD_DONE, "Done");
      assert.notEqual(HOME_WIZARD_DONE, texts.WIZ_FINISH);
    });
  });
});

describe("questions", () => {
  it("system and browser questions", () => {
    assert.equal(texts.WIZ_Q_OS, "Which system are you setting up?");
    assert.equal(texts.WIZ_Q_BROWSER, "Which browser do you use?");
  });

  it("WIZ_QUESTIONS has an entry for every screen with a question, none for join and guide", () => {
    assert.deepEqual(Object.keys(texts.WIZ_QUESTIONS).sort(), ["browser", "connect", "exit", "os", "scope"]);
  });

  it("WIZ_QUESTIONS maps each screen to its text", () => {
    assert.deepEqual(texts.WIZ_QUESTIONS, {
      scope: SCOPE_QUESTION,
      os: "Which system are you setting up?",
      browser: "Which browser do you use?",
      exit: HOME_WIZARD_STEP_EXIT,
      connect: HOME_WIZARD_STEP_CONNECT,
    });
  });

  it("the scope question names the product through NET_NAME", () => {
    assert.equal(texts.WIZ_QUESTIONS.scope, `Where should ${NET_NAME} open sites?`);
  });
});

describe("guide titles and browser names", () => {
  it("WIZ_GUIDE_TITLE puts the name after \"Set up\"", () => {
    assert.equal(texts.WIZ_GUIDE_TITLE("Chrome"), "Set up Chrome");
    assert.equal(texts.WIZ_GUIDE_TITLE("your browser"), "Set up your browser");
  });

  it("BROWSER_NAMES — the tile captions", () => {
    assert.deepEqual(texts.BROWSER_NAMES, { chrome: "Chrome", edge: "Edge", safari: "Safari", firefox: "Firefox", other: "Other" });
  });
});

describe("guide items and warnings", () => {
  it("the copy item, the app link caption and the app download line", () => {
    assert.equal(texts.WIZ_COPY_ITEM, "Copy this address (PAC):");
    assert.equal(texts.WIZ_APP_LINK, "meshtown.apk");
    assert.ok(Object.values(texts).includes(`On the phone, download the ${NET_NAME} app:`), "the app download line is exported");
  });

  it("the warning when no exit is chosen", () => {
    assert.equal(texts.WIZ_EXIT_DIRECT_WARN, "Sites open directly until you choose a member above.");
  });
});

describe("manual setup toggle", () => {
  it("closed with ▸, open with ▾", () => {
    assert.equal(texts.MANUAL_TOGGLE_HAND, "Set up by hand (if the steps above didn't work) ▸");
    assert.equal(texts.MANUAL_TOGGLE_HAND_OPEN, "Set up by hand (if the steps above didn't work) ▾");
  });

  it("the two captions differ only by the arrow", () => {
    assert.equal(texts.MANUAL_TOGGLE_HAND.slice(0, -1), texts.MANUAL_TOGGLE_HAND_OPEN.slice(0, -1));
    assert.notEqual(texts.MANUAL_TOGGLE_HAND.at(-1), texts.MANUAL_TOGGLE_HAND_OPEN.at(-1));
  });
});

describe("all apps on this computer", () => {
  it("the title names the system", () => {
    assert.equal(texts.WIZ_ALL_APPS_TITLE("macOS"), "Set up all apps on macOS");
  });

  it("the browsers block", () => {
    assert.equal(texts.ALL_APPS_BROWSERS_TITLE, "Browsers");
    assert.deepEqual(texts.ALL_APPS_BROWSER_STEPS, [
      "Chrome, Edge, Safari, Opera, Brave and most other browsers use this system setting — there is nothing to change in them. If a browser was open, close it completely and open it again.",
      "Firefox: open \"Settings\" → \"General\" → \"Network Settings\" → \"Settings…\" and make sure \"Use system proxy settings\" is selected (it is the default).",
      `Opera: turn off its built-in VPN — otherwise sites bypass ${NET_NAME}.`,
    ]);
  });

  it("the small Linux line", () => {
    assert.equal(texts.ALL_APPS_LINUX_SMALL, "On Linux, Chrome and other Chromium-based browsers follow the GNOME or KDE proxy setting.");
  });
});

describe("only in one browser", () => {
  it("WIZ_ONE_BROWSER(name) says the browser uses the system setting and points to all apps", () => {
    assert.equal(
      texts.WIZ_ONE_BROWSER("Chrome"),
      `Chrome has no proxy setting of its own — it always uses the system one. To use ${NET_NAME} in Chrome, set it up for all apps on this computer.`,
    );
  });

  it("the name is substituted in both places", () => {
    const text = texts.WIZ_ONE_BROWSER("Safari");
    assert.ok(text.startsWith("Safari has"), text);
    assert.ok(text.includes(`${NET_NAME} in Safari,`), text);
  });

  it("the text for an unnamed browser (Other)", () => {
    assert.equal(
      texts.WIZ_ONE_BROWSER_OTHER,
      "Most browsers (Opera, Brave, Vivaldi, Arc, Yandex Browser) have no proxy setting of their own and use the system one — set up all apps on this computer. If your browser has its own proxy setting (Firefox-based browsers do), paste the PAC address into it instead.",
    );
  });

  it("the switch button", () => {
    assert.equal(texts.WIZ_TO_SYSTEM, "Set up for all apps");
  });
});

describe("the product name", () => {
  for (const file of ["wizard-texts.ts", "../../common/pac-texts.ts"]) {
    it(`${file.split("/").at(-1)} never spells the name as a literal outside comments`, () => {
      const source = fs.readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8");
      const code = source.replace(/\/\*[\s\S]*?\*\//g, "").split("\n").map((l) => l.replace(/^\s*\/\/.*$/, "")).join("\n");
      assert.ok(!code.includes(NET_NAME), `"${NET_NAME}" must come from NET_NAME`);
    });
  }

  it("the texts that mention the product use the current NET_NAME", () => {
    assert.ok(texts.WIZ_ONE_BROWSER("Edge").includes(NET_NAME));
    assert.ok(texts.ALL_APPS_BROWSER_STEPS[2].includes(NET_NAME));
  });
});
