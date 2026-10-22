/* Texts of the "Connect the browser" card and of stopping the node (`pac-texts.ts`): verbatim values —
 * the card markup, the "Settings" section and the confirmation dialog are checked against them. The product name
 * comes from `NET_NAME`, the rest are literals. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "./words.ts";
import * as texts from "./pac-texts.ts";
import {
  CONFIRM_STOP_TEXT, HOST_COPY_ARIA, LINUX_SMALL, LINUX_STEPS, MAC_SMALL, MAC_STEPS, MANUAL_BROWSER_HINT, MANUAL_BROWSER_HINT_NO_SAFARI, MANUAL_INTRO_BASE,
  MANUAL_TAIL_LINUX, MANUAL_TAIL_MAC, MANUAL_TAIL_WINDOWS, MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN, OS_LABEL, WIZARD_AGAIN,
  PAC_ADDR_LABEL, PAC_COPY_ARIA, PAC_COPY_FALLBACK, PAC_INTRO, PAC_NO_EXIT_NOTE, PAC_TITLE, PAC_WHY, PHONE_SOON, STOPPED_NOTE,
  WINDOWS_SOON,
} from "./pac-texts.ts";

describe("card header", () => {
  it("PAC_TITLE — \"Connect the browser to <product name>\"", () => {
    assert.equal(PAC_TITLE, "Connect the browser to " + NET_NAME);
  });

  it("PAC_INTRO — enter the address once, no password needed", () => {
    assert.equal(
      PAC_INTRO,
      `Enter this address once in your computer's proxy settings. After that, the browser opens sites through ${NET_NAME} by itself whenever you choose an exit. No password needed.`,
    );
  });
});

describe("auto-config address", () => {
  it("PAC_ADDR_LABEL — label before the address", () => {
    assert.equal(PAC_ADDR_LABEL, "Auto-config address (PAC):");
  });

  it("PAC_WHY — why the address is needed", () => {
    assert.equal(PAC_WHY, `This is the address the browser uses to find out which sites to open through ${NET_NAME}.`);
  });

  it("PAC_COPY_FALLBACK — clipboard unavailable: without the words \"in the step\"", () => {
    assert.equal(PAC_COPY_FALLBACK, "Clipboard unavailable — select the address and copy it manually.");
    assert.ok(!PAC_COPY_FALLBACK.includes("in the step"), "the PAC address is outside the steps, there are no words \"in the step\"");
  });

  it("PAC_NO_EXIT_NOTE — while \"Directly\" is chosen, sites open directly", () => {
    assert.equal(PAC_NO_EXIT_NOTE, `While "Exit via…" is set to "Directly, without an exit", sites open directly. That's expected.`);
  });
});

describe("system choice", () => {
  it("OS_LABEL — label of the button group", () => {
    assert.equal(OS_LABEL, "Your system:");
  });

  it("WINDOWS_SOON — no Windows steps yet, points to manual setup", () => {
    assert.equal(
      WINDOWS_SOON,
      "We are still checking the steps for Windows — for now use \"Not working? Set up manually\" below.",
    );
  });

  it("PHONE_SOON — iPhone and Android coming soon", () => {
    assert.equal(PHONE_SOON, "iPhone and Android are coming soon. For now, connect the browser on your computer.");
  });
});

describe("manual setup", () => {
  it("MANUAL_TOGGLE — expand button with an arrow", () => {
    assert.equal(MANUAL_TOGGLE, "Not working? Set up manually ▸");
    assert.equal(MANUAL_TOGGLE_OPEN, "Not working? Set up manually ▾");
    assert.equal(WIZARD_AGAIN, "Open step-by-step setup");
    assert.equal(MANUAL_BROWSER_HINT, "Firefox is configured separately — choose it below. The steps are the same for Chrome, Edge and Safari.");
  });

  describe("hint above the browser buttons — on macOS with Safari, on other systems without it", () => {
    it("MANUAL_BROWSER_HINT — the old text for macOS, Safari is named", () => {
      assert.equal(MANUAL_BROWSER_HINT, "Firefox is configured separately — choose it below. The steps are the same for Chrome, Edge and Safari.");
      assert.ok(MANUAL_BROWSER_HINT.includes("Safari"));
    });

    it("MANUAL_BROWSER_HINT_NO_SAFARI — for Windows, Linux and phone: Safari is not named", () => {
      assert.equal(MANUAL_BROWSER_HINT_NO_SAFARI, "Firefox is configured separately — choose it below. The steps are the same for Chrome and Edge.");
      assert.ok(!MANUAL_BROWSER_HINT_NO_SAFARI.includes("Safari"), "there is no Safari button outside macOS — nothing to mention");
    });

    it("both hints start with the same Firefox phrase, but they are different texts", () => {
      const firefox = "Firefox is configured separately — choose it below.";
      assert.ok(MANUAL_BROWSER_HINT.startsWith(firefox));
      assert.ok(MANUAL_BROWSER_HINT_NO_SAFARI.startsWith(firefox));
      assert.notEqual(MANUAL_BROWSER_HINT_NO_SAFARI, MANUAL_BROWSER_HINT);
    });
  });

  describe("manual block intro — common part and a per-system tail", () => {
    it("MANUAL_INTRO_BASE — for programs without auto-config: SOCKS5, address and port", () => {
      assert.equal(
        MANUAL_INTRO_BASE,
        "For programs that don't support automatic setup, enter the proxy by hand — type SOCKS5, with the address and port below.",
      );
    });

    it("MANUAL_TAIL_MAC — turn off \"Automatic Proxy Configuration\" if it was turned on by the steps above", () => {
      assert.equal(MANUAL_TAIL_MAC, "First turn off \"Automatic Proxy Configuration\" if you turned it on following the steps above.");
    });

    it("MANUAL_TAIL_LINUX — switch auto-config to manual mode", () => {
      assert.equal(
        MANUAL_TAIL_LINUX,
        "If you turned on auto-config following the steps above, first switch it to manual mode.",
      );
    });

    it("MANUAL_TAIL_LINUX — no mode item name: in GNOME 43 it is \"Automatic\", in 44+ \"Automatically\"", () => {
      assert.ok(!MANUAL_TAIL_LINUX.includes("Automatically"), "the old quoted item is gone");
      assert.ok(!MANUAL_TAIL_LINUX.includes("Automatic"), "and the GNOME 43 form is not quoted either");
    });

    it("MANUAL_TAIL_WINDOWS — no Windows steps above: without the word \"above\" and without menu item names", () => {
      assert.equal(MANUAL_TAIL_WINDOWS, "If you have already turned on proxy auto-config, turn it off first.");
      assert.ok(!MANUAL_TAIL_WINDOWS.includes("above"), "there is nothing to refer to in the Windows steps");
      assert.ok(!MANUAL_TAIL_WINDOWS.includes("\""), "Windows item names are not verified on a real machine — not quoted");
    });

    it("the old shared MANUAL_INTRO is no longer among the exports", () => {
      assert.ok(!("MANUAL_INTRO" in texts), "MANUAL_INTRO was replaced by MANUAL_INTRO_BASE and three tails");
    });
  });
});

describe("labels of the \"Copy address\" buttons for screen readers", () => {
  it("PAC_COPY_ARIA — copies the PAC address", () => {
    assert.equal(PAC_COPY_ARIA, "Copy address of the PAC file");
  });

  it("HOST_COPY_ARIA — copies the proxy address (manual block)", () => {
    assert.equal(HOST_COPY_ARIA, "Copy address of the proxy");
  });

  it("the labels differ, but start with the visible button text \"Copy address\"", () => {
    assert.notEqual(PAC_COPY_ARIA, HOST_COPY_ARIA);
    for (const label of [PAC_COPY_ARIA, HOST_COPY_ARIA]) {
      assert.ok(label.startsWith("Copy address"), "the screen reader name contains the visible label: " + label);
    }
  });
});

describe("stopping the node", () => {
  it("STOPPED_NOTE — what the browser sees while the node is stopped, and how to start again", () => {
    assert.equal(
      STOPPED_NOTE,
      `While ${NET_NAME} is stopped, sites in the browser don't open (local addresses do). After it starts, sites come back in about 15 seconds. To start it again, use the ${NET_NAME} icon near the clock → "Start ${NET_NAME}".`,
    );
  });

  it("CONFIRM_STOP_TEXT — warning in the confirmation dialog", () => {
    assert.equal(
      CONFIRM_STOP_TEXT,
      `Stop ${NET_NAME}? While it is stopped, sites in the browser don't open (local addresses do); after it starts, they come back in about 15 seconds.`,
    );
  });
});

describe("macOS steps", () => {
  it("MAC_STEPS — six steps verbatim (OK in Latin letters, as in the system)", () => {
    assert.deepEqual(MAC_STEPS, [
      "Click \"Copy address\" above.",
      "Open \"System Settings\" → \"Network\" and choose your connection (Wi‑Fi or Ethernet).",
      "Click \"Details…\" and open \"Proxies\".",
      `If \"SOCKS proxy\" is on (for example, from earlier ${NET_NAME} instructions), turn it off.`,
      "Turn on \"Automatic Proxy Configuration\" (in some versions — \"Auto Proxy Configuration\") and paste the address into the \"URL\" field.",
      "Click \"OK\".",
    ]);
  });

  it("MAC_SMALL — two small lines: tied to the connection, and the path for macOS 12", () => {
    assert.deepEqual(MAC_SMALL, [
      "The setting applies to one connection: if you switch between Wi‑Fi and Ethernet, repeat it for the other one.",
      "On macOS 12 and earlier: \"Network\" → \"Advanced…\" → \"Proxies\".",
    ]);
  });
});

describe("Linux steps", () => {
  it("LINUX_STEPS — four steps, steps 1–3 as for GNOME 45 and newer", () => {
    assert.equal(LINUX_STEPS.length, 4, "same number of steps as before");
    assert.deepEqual(LINUX_STEPS.slice(0, 3), [
      "Click \"Copy address\" above.",
      "Open \"Settings\" → \"Network\" → \"Proxy\".",
      "Turn on \"Network Proxy\", choose \"Automatic\" in the \"Configuration\" list and paste the address into the \"Configuration URL\" field.",
    ]);
  });

  it("LINUX_STEPS[3] — \"Save\" in the window header and a caveat: GNOME 44 has no such button", () => {
    assert.equal(
      LINUX_STEPS[3],
      "Click \"Save\" in the window header. GNOME 44 has no such button — the address is applied immediately.",
    );
  });

  it("LINUX_SMALL[0] — GNOME 43 and earlier: the path through the gear, \"Automatic\", \"Configuration URL\", no \"Save\"", () => {
    assert.equal(
      LINUX_SMALL[0],
      "On GNOME 43 and earlier: \"Settings\" → \"Network\" → the \"Network Proxy\" row → the gear; choose \"Automatic\" and paste the address into the \"Configuration URL\" field. There is no \"Save\" button — the address is applied immediately.",
    );
  });

  it("LINUX_SMALL[0] — the old window and field names of GNOME 43 (\"Options…\", \"Automatic configuration URL\") are gone", () => {
    assert.ok(!LINUX_SMALL[0].includes("Options…"));
    assert.ok(!LINUX_SMALL[0].includes("Automatic configuration URL"));
  });

  it("LINUX_SMALL — two small lines: GNOME 43 and earlier, KDE (KDE unchanged)", () => {
    assert.equal(LINUX_SMALL.length, 2);
    assert.equal(
      LINUX_SMALL[1],
      "On KDE: \"System Settings\" → \"Network & Connectivity\" → \"Wi\u2011Fi & Internet\" (in Plasma 5 — \"Network Settings\") → \"Proxy\" → \"Use proxy configuration URL\".",
    );
  });

  it("LINUX_SMALL — \"Wi\u2011Fi\" with a non-breaking hyphen U+2011, as in the macOS steps", () => {
    const kde = LINUX_SMALL[1];
    assert.ok(kde.includes("Wi\u2011Fi"), "the KDE footnote has a non-breaking hyphen");
    assert.ok(!kde.includes("Wi-Fi"), "there is no plain hyphen in \"Wi-Fi\"");
  });
});

describe("non-breaking hyphen in \"Wi\u2011Fi\"", () => {
  it("no card text has \"Wi-Fi\" with an ASCII hyphen", () => {
    const strings = Object.values(texts).flat().filter((v) => typeof v === "string");
    assert.ok(strings.length > 20, "all string exports are collected");
    for (const s of strings) assert.ok(!s.includes("Wi-Fi"), "plain hyphen in: " + s);
  });
});

describe("type of the step constants", () => {
  it("all four arrays are non-empty arrays of strings", () => {
    for (const list of [MAC_STEPS, MAC_SMALL, LINUX_STEPS, LINUX_SMALL]) {
      assert.ok(Array.isArray(list) && list.length > 0, "an array of strings");
      assert.ok(list.every((s) => typeof s === "string" && s.length > 0), "every string is non-empty");
    }
  });
});
