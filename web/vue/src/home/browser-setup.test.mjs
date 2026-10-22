/* Browser setup (`browser-setup.ts`): a port of `07b-browser-setup.js` — choosing the browser and system
 * by userAgent, the steps, the warning; plus parsing the proxy address and the card view. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { PHONE_SOON } from "../common/pac-texts.ts";
import {
  browserSteps, browserWarnText, detectBrowser, detectOs, isAppUserAgent, proxyState, proxyView, splitHostPort,
} from "./browser-setup.ts";
import { pacBlock } from "./pac-setup.ts";
import { PAC_INTRO } from "../common/pac-texts.ts";
import { laptopState, routerGuestState, routerState } from "../testing/nets-states.mjs";

const BROWSER_WARN = "This is a system-wide setting: other programs that use it will also go through Mesh Town.";

const UA = {
  chromeMac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
  edgeWin: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
  safariMac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
  firefoxLinux: "Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0",
  firefoxWin: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
  iphoneSafari: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
  ipadSafari: "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
  ipodSafari: "Mozilla/5.0 (iPod touch; CPU iPhone OS 15_8 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.6 Mobile/15E148 Safari/604.1",
  androidChrome: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36",
  unknown: "curl/8.0",
};

describe("detectBrowser and detectOs", () => {
  const cases = [
    ["chromeMac", "chrome", "mac"],
    ["edgeWin", "edge", "windows"],
    ["safariMac", "safari", "mac"],
    ["firefoxLinux", "firefox", "linux"],
    ["firefoxWin", "firefox", "windows"],
    ["unknown", "firefox", "linux"],
  ];
  for (const [key, browser, os] of cases) {
    it(`${key}: ${browser} / ${os}`, () => {
      assert.equal(detectBrowser(UA[key]), browser);
      assert.equal(detectOs(UA[key]), os);
    });
  }
});

describe("detectOs, phones", () => {
  describe("phone or tablet userAgent", () => {
    for (const key of ["iphoneSafari", "ipadSafari", "ipodSafari", "androidChrome"]) {
      it(`${key}: "phone", not mac ("like Mac OS X") and not linux ("Linux; Android")`, () => {
        assert.equal(detectOs(UA[key]), "phone");
      });
    }
  });

  describe("computer userAgent", () => {
    it("Windows, Macintosh and X11 Linux stay windows, mac and linux", () => {
      assert.equal(detectOs(UA.edgeWin), "windows");
      assert.equal(detectOs(UA.chromeMac), "mac");
      assert.equal(detectOs(UA.firefoxLinux), "linux");
    });
  });
});

describe("browserSteps", () => {
  for (const browser of ["chrome", "edge", "safari", "firefox"]) {
    for (const os of ["mac", "windows", "linux"]) {
      it(`${browser} on ${os}: the steps are non-empty, the value has exactly an address and a port`, () => {
        const steps = browserSteps(browser, os);
        assert.ok(steps.length >= 1);
        const kinds = steps.filter((s) => s.copyKind).map((s) => s.copyKind);
        const noValues = browser === "safari" && os !== "mac";
        assert.deepEqual(kinds, noValues ? [] : ["host", "port"]);
      });
    }
  }

  it("Firefox: SOCKS v5 and DNS through SOCKS", () => {
    const text = browserSteps("firefox", "mac").map((s) => s.text).join("\n");
    assert.match(text, /SOCKS v5/);
    assert.match(text, /Proxy DNS when using SOCKS v5/);
  });

  it("macOS: System Settings → Network → Details → Proxies → SOCKS proxy", () => {
    const text = browserSteps("chrome", "mac").map((s) => s.text).join("\n");
    for (const w of ["System Settings", "\"Network\"", "\"Details…\"", "\"Proxies\"", "\"SOCKS proxy\""]) {
      assert.ok(text.includes(w), w);
    }
  });

  it("Windows: the system HTTP proxy manually", () => {
    const text = browserSteps("edge", "windows").map((s) => s.text).join("\n");
    assert.match(text, /Manual proxy setup/);
    assert.match(text, /Save/);
    assert.doesNotMatch(text, /only HTTP proxy/);
  });

  it("Safari not on macOS — a pointer to another browser", () => {
    assert.match(browserSteps("safari", "linux")[0].text, /only on macOS/);
  });
});

describe("browserSteps, phone", () => {
  it("any browser — one PHONE_SOON line, no values to copy", () => {
    for (const browser of ["chrome", "edge", "safari", "firefox"]) {
      assert.deepEqual(browserSteps(browser, "phone"), [{ text: PHONE_SOON }], browser);
    }
  });
});

describe("warning", () => {
  it("Chrome, Edge, Safari (Safari only on macOS) — present; Firefox — absent", () => {
    for (const b of ["chrome", "edge", "safari"]) assert.equal(browserWarnText(b, "mac"), BROWSER_WARN);
    for (const os of ["linux", "windows"]) {
      for (const b of ["chrome", "edge"]) assert.equal(browserWarnText(b, os), BROWSER_WARN);
    }
    assert.equal(browserWarnText("safari", "linux"), "");
    for (const os of ["mac", "windows", "linux"]) assert.equal(browserWarnText("firefox", os), "");
  });

  it("on Windows — for Chrome and Edge, absent for Firefox and Safari", () => {
    for (const b of ["chrome", "edge"]) assert.equal(browserWarnText(b, "windows"), BROWSER_WARN);
    for (const b of ["firefox", "safari"]) assert.equal(browserWarnText(b, "windows"), "");
  });

  it("the warning text verbatim", () => {
    assert.equal(browserWarnText("chrome", "mac"), BROWSER_WARN);
  });

  it("on a phone — empty for any browser (no steps, we don't change the setting)", () => {
    for (const b of ["chrome", "edge", "safari", "firefox"]) assert.equal(browserWarnText(b, "phone"), "", b);
  });
});

describe("splitHostPort", () => {
  it("a plain address — host and port", () => assert.deepEqual(splitHostPort("127.0.0.1:1080"), { host: "127.0.0.1", port: "1080" }));

  it("IPv6 in brackets — split at the last colon, the brackets are removed", () => {
    assert.deepEqual(splitHostPort("[::1]:1080"), { host: "::1", port: "1080" });
  });

  it("no colon — the whole address is the host, the port is empty, no exception", () => {
    assert.deepEqual(splitHostPort("localhost"), { host: "localhost", port: "" });
  });
});

describe("proxyView", () => {
  const PAC = "http://127.0.0.1:1080/proxy.pac";

  it("proxy off — on: false", () => {
    assert.deepEqual(proxyView({ socks: { enabled: false, listen: null } }), { on: false });
  });

  it("on — on: true, host, port from listen and the PAC address", () => {
    assert.deepEqual(proxyView({ socks: { enabled: true, listen: "127.0.0.1:1080" } }), { on: true, host: "127.0.0.1", port: "1080", pacUrl: PAC });
  });

  it("lan present — goes into the view; absent or null — no lan key", () => {
    const on = { enabled: true, listen: "127.0.0.1:1080" };
    assert.deepEqual(proxyView({ socks: { ...on, lan: [{ addr: "192.168.1.7:1080", iface: "br-lan" }] } }), { on: true, host: "127.0.0.1", port: "1080", pacUrl: PAC, lan: [{ addr: "192.168.1.7:1080", iface: "br-lan" }] });
    assert.deepEqual(proxyView({ socks: { ...on, lan: [] } }), { on: true, host: "127.0.0.1", port: "1080", pacUrl: PAC });
  });

  it("no listen — treated as off", () => {
    assert.deepEqual(proxyView({ socks: { listen: null } }), { on: false });
    assert.deepEqual(proxyView({}), { on: false });
  });

  describe("PAC address (pacUrl)", () => {
    it("the node sent socks.pac_url — it is used, not one built from listen", () => {
      const socks = { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/custom/proxy.pac" };
      assert.equal(proxyView({ socks }).pacUrl, "http://127.0.0.1:1080/custom/proxy.pac");
    });

    it("no pac_url field (an old snapshot) — \"http://\" + listen + \"/proxy.pac\"", () => {
      assert.equal(proxyView({ socks: { enabled: true, listen: "127.0.0.1:1080" } }).pacUrl, PAC);
    });

    it("pac_url empty or null — same as no field", () => {
      for (const pac_url of ["", null]) {
        const socks = { enabled: true, listen: "127.0.0.1:1080", pac_url };
        assert.equal(proxyView({ socks }).pacUrl, PAC, `pac_url: ${JSON.stringify(pac_url)}`);
      }
    });

    it("IPv6 listen \"[::1]:1080\" — the brackets stay in the address: http://[::1]:1080/proxy.pac, while the host is without brackets", () => {
      const v = proxyView({ socks: { enabled: true, listen: "[::1]:1080" } });
      assert.equal(v.pacUrl, "http://[::1]:1080/proxy.pac");
      assert.equal(v.host, "::1");
      assert.equal(v.port, "1080");
    });

    it("proxy off although pac_url was sent — {on: false}, no pacUrl", () => {
      const socks = { enabled: false, listen: null, pac_url: "http://127.0.0.1:1080/proxy.pac" };
      assert.deepEqual(proxyView({ socks }), { on: false });
    });
  });
});

describe("proxyState", () => {
  const on = { socks: { enabled: true, listen: "127.0.0.1:1080" } };

  it("proxy off or no snapshot — only { on: false }", () => {
    assert.deepEqual(proxyState({ socks: { enabled: false } }, "chrome", UA.chromeMac), { on: false });
    assert.deepEqual(proxyState(null, null, UA.chromeMac), { on: false });
  });

  it("no choice — browser and system by userAgent, steps and address", () => {
    const v = proxyState(on, null, UA.firefoxLinux);
    assert.equal(v.on, true);
    assert.equal(v.browser, "firefox");
    assert.equal(v.host, "127.0.0.1");
    assert.deepEqual(v.steps, browserSteps("firefox", "linux"));
    assert.equal(v.warn, browserWarnText("firefox", "linux"));
  });

  it("the user's choice beats userAgent, the system comes from userAgent", () => {
    const v = proxyState(on, "safari", UA.chromeMac);
    assert.equal(v.browser, "safari");
    assert.deepEqual(v.steps, browserSteps("safari", "mac"));
  });

  it("Safari not on macOS — the \"only on macOS\" step, no warning", () => {
    const v = proxyState(on, "safari", UA.firefoxWin);
    assert.match(v.steps[0].text, /only on macOS/);
    assert.equal(v.warn, "");
  });

  it("the PAC address from the snapshot goes into the view along with host and port", () => {
    const v = proxyState({ socks: { ...on.socks, pac_url: "http://127.0.0.1:1080/proxy.pac" } }, null, UA.chromeMac);
    assert.equal(v.pacUrl, "http://127.0.0.1:1080/proxy.pac");
  });

  describe("system (os) and PAC block (pac)", () => {
    it("no system choice — os by userAgent, pac is this system's block", () => {
      const v = proxyState(on, null, UA.firefoxLinux);
      assert.equal(v.os, "linux");
      assert.deepEqual(v.pac, pacBlock("linux"));
    });

    it("the system choice beats userAgent: Windows was chosen on a Mac", () => {
      const v = proxyState(on, null, UA.chromeMac, "windows");
      assert.equal(v.os, "windows");
      assert.deepEqual(v.pac, pacBlock("windows"));
    });

    it("osChoice = null — same as \"no choice\"", () => {
      assert.equal(proxyState(on, null, UA.edgeWin, null).os, "windows");
    });

    it("phone userAgent and no choice — os \"phone\", pac with a stub, steps are PHONE_SOON, no warning", () => {
      const v = proxyState(on, null, UA.iphoneSafari);
      assert.equal(v.os, "phone");
      assert.deepEqual(v.pac, pacBlock("phone"));
      assert.deepEqual(v.steps, [{ text: PHONE_SOON }]);
      assert.equal(v.warn, "");
    });

    it("\"Phone\" was chosen on a computer — manual steps and warning as for a phone", () => {
      const v = proxyState(on, "chrome", UA.chromeMac, "phone");
      assert.equal(v.os, "phone");
      assert.deepEqual(v.steps, [{ text: PHONE_SOON }]);
      assert.equal(v.warn, "");
    });

    it("Windows was chosen on a Mac — manual steps and warning of Windows, not macOS", () => {
      const v = proxyState(on, "chrome", UA.chromeMac, "windows");
      assert.deepEqual(v.steps, browserSteps("chrome", "windows"));
      assert.notDeepEqual(v.steps, browserSteps("chrome", "mac"));
      assert.equal(v.warn, browserWarnText("chrome", "windows"));
    });

    it("on a real Windows with no explicit choice — Windows steps, as before", () => {
      assert.deepEqual(proxyState(on, null, UA.edgeWin).steps, browserSteps("edge", "windows"));
    });

    it("the browser choice doesn't depend on the system choice", () => {
      const v = proxyState(on, null, UA.chromeMac, "linux");
      assert.equal(v.browser, "chrome");
    });
  });
});

describe("router and network choice: the auto-config address and paragraph", () => {
  const CHROME = UA.chromeMac;
  // The constant PAC_INTRO_ROUTER is checked in nets-texts.test.mjs.
  const PAC_INTRO_ROUTER = "Enter this address once in the proxy settings of a phone or computer connected to the router. After that, the browser opens sites through Mesh Town by itself whenever you choose an exit. No password needed.";

  describe("router, the proxy is open to the home network", () => {
    it("proxyView.pacUrl — the home network address from socks.lan, not socks.pac_url", () => {
      const state = routerState();
      state.socks.pac_url = "http://127.0.0.1:1080/proxy.pac";
      assert.equal(proxyView(state).pacUrl, "http://192.168.88.1:1080/proxy.pac");
    });

    it("proxyState: the paragraph is about a phone or computer connected to the router; the manual block's \"Server address\" is the same host", () => {
      const view = proxyState(routerState(), null, CHROME);
      assert.equal(view.intro, PAC_INTRO_ROUTER);
      assert.equal(view.pacUrl, "http://192.168.88.1:1080/proxy.pac");
    });

    it("several open networks — the \"Home network\" is taken", () => {
      const state = routerGuestState({ ifaces: ["br-guest", "br-lan"] });
      state.socks.lan = [{ addr: "192.168.9.1:1080", iface: "br-guest" }, { addr: "192.168.88.1:1080", iface: "br-lan" }];
      assert.equal(proxyView(state).pacUrl, "http://192.168.88.1:1080/proxy.pac");
    });

    it("the node gave the pac_url of the request's network (not loopback) — the page shows it, the manual block's server address is its host", () => {
      const state = routerGuestState({ ifaces: ["br-guest", "br-lan"] });
      state.socks.pac_url = "http://192.168.9.1:1080/proxy.pac";
      const view = proxyState(state, null, CHROME);
      assert.equal(view.pacUrl, "http://192.168.9.1:1080/proxy.pac");
      assert.equal(view.host, "192.168.9.1");
      assert.equal(view.intro, PAC_INTRO_ROUTER);
    });
  });

  describe("previous behavior", () => {
    it("router, the proxy only for this computer: the address from socks.pac_url, the paragraph PAC_INTRO", () => {
      const view = proxyState(routerState({ access: "localhost" }), null, CHROME);
      assert.equal(view.pacUrl, "http://127.0.0.1:1080/proxy.pac");
      assert.equal(view.intro, PAC_INTRO);
    });

    it("a laptop, even with the proxy for the network: the address from socks.pac_url, the paragraph PAC_INTRO", () => {
      const view = proxyState(laptopState({ access: "localnet" }), null, CHROME);
      assert.equal(view.pacUrl, "http://127.0.0.1:1080/proxy.pac");
      assert.equal(view.intro, PAC_INTRO);
    });
  });
});

describe("isAppUserAgent", () => {
  it("recognizes the Android app suffix only", () => {
    assert.equal(isAppUserAgent("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android"), true);
    assert.equal(isAppUserAgent("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile"), false);
    assert.equal(isAppUserAgent(""), false);
  });
});
