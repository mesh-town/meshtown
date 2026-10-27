/* Texts of the "Connect the browser" card and of stopping the node (`pac-texts.ts`): verbatim values —
 * the card markup, the "Settings" section and the confirmation dialog are checked against them. The product name
 * comes from `NET_NAME`, the rest are literals. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { NET_NAME } from "./words.ts";
import * as texts from "./pac-texts.ts";
import {
  ANDROID_INTRO, ANDROID_STEPS, IOS_NO_APP,
  CONFIRM_STOP_TEXT, HOST_COPY_ARIA, LINUX_SMALL, LINUX_STEPS, MAC_SMALL, MAC_STEPS, MANUAL_BROWSER_HINT, MANUAL_BROWSER_HINT_NO_SAFARI, MANUAL_INTRO_BASE,
  MANUAL_TAIL_LINUX, MANUAL_TAIL_MAC, MANUAL_TAIL_WINDOWS, MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN, OS_LABEL, WIZARD_AGAIN,
  PAC_ADDR_LABEL, PAC_COPY_ARIA, PAC_COPY_FALLBACK, PAC_INTRO, PAC_NO_EXIT_NOTE, PAC_TITLE, PAC_WHY, STOPPED_NOTE,
  WINDOWS_SMALL, WINDOWS_STEPS,
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
    assert.equal(PAC_ADDR_LABEL, "Setup address (PAC):");
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

  it("the old placeholders for Windows and phones are no longer among the exports", () => {
    assert.ok(!("WINDOWS_SOON" in texts), "Windows has real steps now");
    assert.ok(!("PHONE_SOON" in texts), "the phone is split into Android and iOS");
  });
});

describe("Windows steps", () => {
  it("WINDOWS_STEPS — four steps verbatim (the copy-address step is drawn by the wizard)", () => {
    assert.deepEqual(WINDOWS_STEPS, [
      "Open \"Settings\" → \"Network & internet\" → \"Proxy\".",
      "Next to \"Use setup script\", click \"Set up\".",
      "Turn on \"Use setup script\", paste the address into \"Script address\" and click \"Save\".",
      "If \"Use a proxy server\" under \"Manual proxy setup\" is on (for example, from earlier " + NET_NAME + " instructions), turn it off.",
    ]);
  });

  it("WINDOWS_SMALL — one small line for Windows 10", () => {
    assert.deepEqual(WINDOWS_SMALL, [
      "On Windows 10: \"Settings\" → \"Network & Internet\" → \"Proxy\" — turn on \"Use setup script\", paste the address into \"Script address\" and click \"Save\".",
    ]);
  });
});

describe("phone texts", () => {
  it("ANDROID_INTRO — on Android the phone joins by itself", () => {
    assert.equal(
      ANDROID_INTRO,
      `On Android, ${NET_NAME} is an app — the phone joins the network by itself and doesn't need this computer.`,
    );
  });

  it("ANDROID_STEPS — four steps after the app download line, verbatim", () => {
    assert.deepEqual(ANDROID_STEPS, [
      "Open the downloaded file. If Android asks, allow installing apps from your browser.",
      "On this computer, create an invite for the phone: \"Network invite\" below → \"Invite a member\". Send it to the phone — each invite works once.",
      "In the app, tap \"Join with an invite\" and paste the invite.",
      "Choose an exit and tap \"Connect\" at the bottom of the screen.",
    ]);
  });

  it("IOS_NO_APP — honest line, no steps through the computer", () => {
    assert.equal(
      IOS_NO_APP,
      `There is no ${NET_NAME} app for iPhone and iPad yet. For now, use ${NET_NAME} on a computer or an Android phone.`,
    );
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
    it("MANUAL_INTRO_BASE — for programs without the setup address: server, port, SOCKS5", () => {
      assert.equal(
        MANUAL_INTRO_BASE,
        "Some programs can't use the setup address. Enter the server and port by hand, proxy type \"SOCKS5\". First turn off the setup address if you entered it above.",
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
      `While ${NET_NAME} is stopped, sites in the browser open directly, without an exit; in strict mode they don't open at all (local addresses do). After it starts, everything comes back in about 15 seconds. To start it again, use the ${NET_NAME} icon near the clock → "Start ${NET_NAME}".`,
    );
  });

  it("the stop texts say nothing about forbidden or unblocked sites", () => {
    for (const t of [STOPPED_NOTE, texts.STOPPED_NOTE_APP, CONFIRM_STOP_TEXT]) assert.ok(!/vpn|block|bypass/i.test(t), t);
  });

  it("CONFIRM_STOP_TEXT — warning in the confirmation dialog", () => {
    assert.equal(
      CONFIRM_STOP_TEXT,
      `Stop ${NET_NAME}? While it is stopped, sites in the browser open directly, without an exit; in strict mode they don't open at all (local addresses do). After it starts, everything comes back in about 15 seconds.`,
    );
  });
});

describe("proxy scope choice", () => {
  it("the question and the two answers", () => {
    assert.equal(texts.SCOPE_QUESTION, `Where should ${NET_NAME} open sites?`);
    assert.equal(texts.SCOPE_SYSTEM, "All apps on this computer (recommended)");
    assert.equal(texts.SCOPE_BROWSER, "Only in one browser");
  });

  it("FIREFOX_STEPS — Settings, Network Settings, the automatic URL, OK; no copy step", () => {
    assert.deepEqual(texts.FIREFOX_STEPS, [
      "In Firefox open \"Settings\" → \"General\" and scroll down to \"Network Settings\".",
      "Click \"Settings…\".",
      "Choose \"Automatic proxy configuration URL\" and paste the address into the field below it.",
      "Click \"OK\".",
    ]);
  });

  it("BROWSER_SCOPE_NOTE — other browsers follow the system setting", () => {
    assert.equal(texts.BROWSER_SCOPE_NOTE, "Chrome, Edge and Safari use the system setting — for them choose \"In the whole system\".");
  });
});

describe("macOS steps", () => {
  it("MAC_STEPS — five steps verbatim, no copy step (OK in Latin letters, as in the system)", () => {
    assert.deepEqual(MAC_STEPS, [
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
  it("LINUX_STEPS — three steps, no copy step, steps 1–2 as for GNOME 45 and newer", () => {
    assert.equal(LINUX_STEPS.length, 3, "one less than before: the copy step moved out of the list");
    assert.deepEqual(LINUX_STEPS.slice(0, 2), [
      "Open \"Settings\" → \"Network\" → \"Proxy\".",
      "Turn on \"Network Proxy\", choose \"Automatic\" in the \"Configuration\" list and paste the address into the \"Configuration URL\" field.",
    ]);
  });

  it("LINUX_STEPS[2] — \"Save\" in the window header and a caveat: GNOME 44 has no such button", () => {
    assert.equal(
      LINUX_STEPS[2],
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
  it("all step arrays are non-empty arrays of strings", () => {
    for (const list of [MAC_STEPS, MAC_SMALL, LINUX_STEPS, LINUX_SMALL, WINDOWS_STEPS, ANDROID_STEPS, texts.FIREFOX_STEPS]) {
      assert.ok(Array.isArray(list) && list.length > 0, "an array of strings");
      assert.ok(list.every((s) => typeof s === "string" && s.length > 0), "every string is non-empty");
    }
  });

  it("no OS step list starts with the copy-address step (the card draws its own address row)", () => {
    for (const list of [MAC_STEPS, LINUX_STEPS, texts.FIREFOX_STEPS, WINDOWS_STEPS, ANDROID_STEPS]) {
      assert.ok(!list[0].includes("Copy address"), "first step: " + list[0]);
    }
  });
});
