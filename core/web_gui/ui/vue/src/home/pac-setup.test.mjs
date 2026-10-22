/* Browser connection steps by the PAC address (`pac-setup.ts`): the list of systems for the switcher,
 * the text block (steps, small lines, placeholder) for the chosen system, availability and the intro of
 * the manual block by system, the list of browser buttons and the hint above them by system (Safari
 * only on macOS), the copy button caption for screen readers. Pure functions. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  HOST_COPY_ARIA, LINUX_SMALL, LINUX_STEPS, MAC_SMALL, MAC_STEPS, MANUAL_BROWSER_HINT, MANUAL_BROWSER_HINT_NO_SAFARI, MANUAL_INTRO_BASE, MANUAL_TAIL_LINUX,
  MANUAL_TAIL_MAC, MANUAL_TAIL_WINDOWS, PAC_COPY_ARIA, PHONE_SOON, WINDOWS_SOON,
} from "../common/pac-texts.ts";
import { BROWSER_IDS } from "./browser-setup.ts";
import { browserList, copyAriaLabel, manualAvailable, manualBrowserHint, manualIntro, PAC_OS_LIST, pacBlock } from "./pac-setup.ts";

const WITH_SAFARI = [["chrome", "Chrome"], ["edge", "Edge"], ["safari", "Safari"], ["firefox", "Firefox"]];
const WITHOUT_SAFARI = [["chrome", "Chrome"], ["edge", "Edge"], ["firefox", "Firefox"]];

describe("PAC_OS_LIST", () => {
  it("four systems in the order macOS, Windows, Linux, Phone — id and button caption", () => {
    assert.deepEqual(PAC_OS_LIST, [
      ["mac", "macOS"],
      ["windows", "Windows"],
      ["linux", "Linux"],
      ["phone", "Phone"],
    ]);
  });
});

describe("pacBlock", () => {
  describe("macOS", () => {
    it("MAC_STEPS steps, MAC_SMALL small lines, no placeholder", () => {
      assert.deepEqual(pacBlock("mac"), { steps: MAC_STEPS, small: MAC_SMALL, note: "" });
    });
  });

  describe("Linux", () => {
    it("LINUX_STEPS steps, LINUX_SMALL small lines, no placeholder", () => {
      assert.deepEqual(pacBlock("linux"), { steps: LINUX_STEPS, small: LINUX_SMALL, note: "" });
    });
  });

  describe("Windows", () => {
    it("no steps or small lines, the WINDOWS_SOON placeholder", () => {
      assert.deepEqual(pacBlock("windows"), { steps: [], small: [], note: WINDOWS_SOON });
    });
  });

  describe("Phone", () => {
    it("no steps or small lines, the PHONE_SOON placeholder", () => {
      assert.deepEqual(pacBlock("phone"), { steps: [], small: [], note: PHONE_SOON });
    });
  });

  describe("each of the four systems", () => {
    it("gives exactly the keys steps, small, note: arrays of strings and a string", () => {
      for (const [os] of PAC_OS_LIST) {
        const block = pacBlock(os);
        assert.deepEqual(Object.keys(block).sort(), ["note", "small", "steps"], os);
        assert.ok(Array.isArray(block.steps) && Array.isArray(block.small), os);
        assert.equal(typeof block.note, "string", os);
      }
    });

    it("either steps or a placeholder — not both", () => {
      for (const [os] of PAC_OS_LIST) {
        const block = pacBlock(os);
        assert.equal(block.steps.length > 0, block.note === "", os + ": there are steps if and only if there is no placeholder");
      }
    });
  });
});

describe("manualAvailable", () => {
  it("macOS, Windows, Linux — the manual block exists", () => {
    for (const os of ["mac", "windows", "linux"]) assert.equal(manualAvailable(os), true, os);
  });

  it("phone — no manual block (there are no steps for the phone, \"soon\" is already said above)", () => {
    assert.equal(manualAvailable("phone"), false);
  });

  it("false for exactly one system of four", () => {
    const off = PAC_OS_LIST.map(([os]) => os).filter((os) => !manualAvailable(os));
    assert.deepEqual(off, ["phone"]);
  });
});

describe("manualIntro", () => {
  describe("macOS", () => {
    it("common part, a space and the tail about \"Automatic Proxy Configuration\"", () => {
      assert.equal(manualIntro("mac"), MANUAL_INTRO_BASE + " " + MANUAL_TAIL_MAC);
      assert.match(manualIntro("mac"), /Automatic Proxy Configuration.*steps above\.$/);
    });
  });

  describe("Linux", () => {
    it("common part, a space and the tail about switching auto-config to manual mode", () => {
      assert.equal(manualIntro("linux"), MANUAL_INTRO_BASE + " " + MANUAL_TAIL_LINUX);
      assert.equal(
        manualIntro("linux"),
        "For programs that don't support automatic setup, enter the proxy by hand — type SOCKS5, with the address and port below. " +
          "If you turned on auto-config following the steps above, first switch it to manual mode.",
      );
    });

    it("the mode item name is not quoted: GNOME 43 has \"Automatic\", 44+ has \"Automatically\"", () => {
      assert.ok(!manualIntro("linux").includes("Automatically"), manualIntro("linux"));
      assert.ok(!manualIntro("linux").includes("Automatic"), manualIntro("linux"));
    });
  });

  describe("Windows", () => {
    it("common part, a space and the tail about auto-config already turned on", () => {
      assert.equal(manualIntro("windows"), MANUAL_INTRO_BASE + " " + MANUAL_TAIL_WINDOWS);
    });

    it("no word \"above\": there are no Windows steps above the manual block", () => {
      assert.ok(!manualIntro("windows").includes("above"), manualIntro("windows"));
    });
  });

  describe("Phone", () => {
    it("only the common part, no tail (the block is hidden, the function still answers)", () => {
      assert.equal(manualIntro("phone"), MANUAL_INTRO_BASE);
    });
  });

  describe("each of the four systems", () => {
    it("starts with the common part: enter the proxy manually — SOCKS5, address and port", () => {
      for (const [os] of PAC_OS_LIST) {
        assert.ok(manualIntro(os).startsWith("For programs that don't support automatic setup, enter the proxy by hand"), os);
        assert.ok(manualIntro(os).includes("SOCKS5"), os);
      }
    });

    it("three desktop systems give three different intros", () => {
      const texts = new Set(["mac", "linux", "windows"].map(manualIntro));
      assert.equal(texts.size, 3);
    });
  });
});

describe("browserList", () => {
  describe("macOS", () => {
    it("four browsers in the order Chrome, Edge, Safari, Firefox — id and button caption", () => {
      assert.deepEqual(browserList("mac"), WITH_SAFARI);
    });
  });

  describe("Windows, Linux, Phone", () => {
    it("no Safari (it exists only on macOS), the other three in the same order: Chrome, Edge, Firefox", () => {
      for (const os of ["windows", "linux", "phone"]) assert.deepEqual(browserList(os), WITHOUT_SAFARI, os);
    });

    it("id \"safari\" is absent among the buttons", () => {
      for (const os of ["windows", "linux", "phone"]) {
        assert.ok(!browserList(os).some(([id]) => id === "safari"), os);
        assert.ok(!browserList(os).some(([, name]) => name === "Safari"), os);
      }
    });
  });

  describe("each of the four systems", () => {
    it("Safari only on macOS: exactly one system of four", () => {
      const withSafari = PAC_OS_LIST.map(([os]) => os).filter((os) => browserList(os).some(([id]) => id === "safari"));
      assert.deepEqual(withSafari, ["mac"]);
    });

    it("button ids are from BROWSER_IDS, without repeats, Firefox is always last", () => {
      for (const [os] of PAC_OS_LIST) {
        const ids = browserList(os).map(([id]) => id);
        assert.ok(ids.every((id) => BROWSER_IDS.includes(id)), os + ": " + ids);
        assert.equal(new Set(ids).size, ids.length, os + ": repeats");
        assert.equal(ids[ids.length - 1], "firefox", os);
      }
    });

    it("Chrome, Edge and Firefox are on all four", () => {
      for (const [os] of PAC_OS_LIST) {
        const ids = browserList(os).map(([id]) => id);
        for (const id of ["chrome", "edge", "firefox"]) assert.ok(ids.includes(id), os + ": no " + id);
      }
    });
  });
});

describe("manualBrowserHint", () => {
  it("macOS — the usual MANUAL_BROWSER_HINT (about Chrome, Edge and Safari)", () => {
    assert.equal(manualBrowserHint("mac"), MANUAL_BROWSER_HINT);
    assert.ok(manualBrowserHint("mac").includes("Safari"));
  });

  it("Windows, Linux, Phone — MANUAL_BROWSER_HINT_NO_SAFARI (about Chrome and Edge)", () => {
    for (const os of ["windows", "linux", "phone"]) {
      assert.equal(manualBrowserHint(os), MANUAL_BROWSER_HINT_NO_SAFARI, os);
      assert.ok(!manualBrowserHint(os).includes("Safari"), os);
    }
  });

  it("the hint names Safari if and only if the Safari button is in browserList", () => {
    for (const [os] of PAC_OS_LIST) {
      const hasButton = browserList(os).some(([id]) => id === "safari");
      assert.equal(manualBrowserHint(os).includes("Safari"), hasButton, os);
    }
  });
});

describe("copyAriaLabel", () => {
  const NOTHING_COPIED = [null, undefined, "", false];

  describe("PAC address (pac)", () => {
    it("nothing copied — PAC_COPY_ARIA", () => {
      for (const copied of NOTHING_COPIED) assert.equal(copyAriaLabel("pac", copied), PAC_COPY_ARIA, String(copied));
    });

    it("exactly the PAC address copied — \"Copied\" (a static caption would hide the result from a screen reader)", () => {
      assert.equal(copyAriaLabel("pac", "pac"), "Copied");
    });

    it("something else copied (host, port, lan) — the caption is as before", () => {
      for (const copied of ["host", "port", "lan"]) assert.equal(copyAriaLabel("pac", copied), PAC_COPY_ARIA, copied);
    });
  });

  describe("proxy address in the manual block (host)", () => {
    it("nothing copied — HOST_COPY_ARIA", () => {
      for (const copied of NOTHING_COPIED) assert.equal(copyAriaLabel("host", copied), HOST_COPY_ARIA, String(copied));
    });

    it("exactly the host copied — \"Copied\"", () => {
      assert.equal(copyAriaLabel("host", "host"), "Copied");
    });

    it("something else copied (PAC, port, lan) — the caption is as before", () => {
      for (const copied of ["pac", "port", "lan"]) assert.equal(copyAriaLabel("host", copied), HOST_COPY_ARIA, copied);
    });
  });

  describe("port and address for the network (port, lan)", () => {
    it("no attribute (undefined) — the name comes from the button's visible text, in any copy state", () => {
      for (const value of ["port", "lan"]) {
        for (const copied of [...NOTHING_COPIED, "pac", "host", "port", "lan"]) {
          assert.equal(copyAriaLabel(value, copied), undefined, value + " when copied=" + String(copied));
        }
      }
    });
  });

  it("the two \"Copy address\" captions differ in the same state", () => {
    for (const copied of [null, "port", "lan"]) {
      assert.notEqual(copyAriaLabel("pac", copied), copyAriaLabel("host", copied), "copied=" + String(copied));
    }
  });
});
