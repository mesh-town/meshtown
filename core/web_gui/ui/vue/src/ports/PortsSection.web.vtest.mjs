/* "Ports shared with you": a member's port as a link at its `.meshtown` address, the "Copy" button, ports a browser refuses,
 * members with the same name, the column set and the hints. One network: rows come straight from `ports.network`. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const URL_80 = "http://finch.meshtown";
const URL_8080 = "http://finch.meshtown:8080";
const NOT_FOR_BROWSER_TITLE = "Browsers don't open this port. Use “Open on this computer” and a program for it.";
const SAME_NAME_TITLE = "Another member has the same name, so this address ends with the start of this member's id.";
const SAME_NAME_HINT = "Some members have the same name, so their addresses end with the start of the member's id.";
const OPEN_TITLE = "Make this port available at 127.0.0.1 for programs other than the browser";

let mounted;
let written;

afterEach(() => {
  mounted.wrapper.unmount();
  api.mockReset();
  Reflect.deleteProperty(navigator, "clipboard");
});

describe("the table", () => {
  it("is titled \"Ports shared with you\" and has the columns Member, Port, In the browser, In other programs", () => {
    const { wrapper } = when_shown([]);

    expect(wrapper.get("table[aria-label='Ports shared with you']").exists()).toBe(true);
    expect(wrapper.text()).toContain("Ports shared with you");
    expect(headers(wrapper)).toEqual(["Member", "Port", "In the browser", "In other programs"]);
  });

  it("without rows says that no member has shared ports yet", () => {
    const { wrapper } = when_shown([]);

    expect(wrapper.get("#ports-net-tbody").text()).toBe("— no member has shared ports with you yet");
  });
});

describe("the member cell", () => {
  it("has the member's name and the \"online\" badge", () => {
    const { wrapper } = when_shown([row({ online: true })]);

    const cell = cellsOf(wrapper)[0];
    expect(cell.text()).toContain("finch");
    expect(cell.get(".badge").text()).toBe("online");
  });

  it("of an offline member has the \"offline\" badge", () => {
    const { wrapper } = when_shown([row({ online: false })]);

    expect(cellsOf(wrapper)[0].get(".badge").text()).toBe("offline");
  });
});

describe("the port cell", () => {
  it("has the number and, under it, the port's name", () => {
    const { wrapper } = when_shown([row({ port: 8080, name: "router page" })]);

    const cell = cellsOf(wrapper)[1];
    expect(cell.get(".mono").text()).toBe("8080");
    expect(cell.text()).toContain("router page");
  });

  it("of a port without a name has only the number", () => {
    const { wrapper } = when_shown([row({ port: 8080, name: "" })]);

    expect(cellsOf(wrapper)[1].text()).toBe("8080");
  });
});

describe("the address in the browser column", () => {
  it("is a link to web_url that opens in a new tab and shows the address itself", () => {
    const { wrapper } = when_shown([row({ web_url: URL_8080 })]);

    const link = cellsOf(wrapper)[2].get("a");
    expect(link.attributes("href")).toBe(URL_8080);
    expect(link.attributes("target")).toBe("_blank");
    expect(link.attributes("rel")).toBe("noopener");
    expect(link.classes()).toContain("mono");
    expect(link.text()).toBe(URL_8080);
  });

  it("of port 80 is shown without a port, of port 443 with https", () => {
    const { wrapper } = when_shown([row({ port: 80, web_url: URL_80 }), row({ node: "cd", port: 443, web_url: "https://finch.meshtown" })]);

    const links = wrapper.findAll("#ports-net-tbody a");
    expect(links.map((a) => a.text())).toEqual([URL_80, "https://finch.meshtown"]);
  });

  it("has a \"Copy\" button named \"Copy address <url>\"", () => {
    const { wrapper } = when_shown([row({ web_url: URL_8080 })]);

    const button = copyButton(wrapper, URL_8080);
    expect(button.exists()).toBe(true);
    expect(button.text()).toBe("Copy");
  });

  it("is copied by the button, which then says \"Copied\"", async () => {
    const { wrapper } = when_shown([row({ web_url: URL_8080 })], { clipboard: "works" });

    await copyButton(wrapper, URL_8080).trigger("click");
    await flushPromises();

    expect(written).toEqual([URL_8080]);
    expect(copyButton(wrapper, URL_8080).text()).toBe("Copied");
  });

  it("is not reported as copied when the clipboard refuses", async () => {
    const { wrapper } = when_shown([row({ web_url: URL_8080 })], { clipboard: "refuses" });

    await copyButton(wrapper, URL_8080).trigger("click");
    await flushPromises();

    expect(copyButton(wrapper, URL_8080).text()).toBe("Copy");
  });

  it("is empty, with neither a link nor a \"Copy\" button, when web_url is null", () => {
    const { wrapper } = when_shown([row({ web_url: null })]);

    const cell = cellsOf(wrapper)[2];
    expect(cell.find("a").exists()).toBe(false);
    expect(cell.find("button").exists()).toBe(false);
    expect(cell.text()).not.toContain("not for the browser");
  });
});

describe("a port the browser refuses", () => {
  it("says \"not for the browser\" with an explaining title instead of a link", () => {
    const { wrapper } = when_shown([row({ port: 22, web_url: "http://finch.meshtown:22" })]);

    const cell = cellsOf(wrapper)[2];
    expect(cell.text()).toBe("not for the browser");
    expect(cell.get("[title]").attributes("title")).toBe(NOT_FOR_BROWSER_TITLE);
    expect(cell.find("a").exists()).toBe(false);
    expect(cell.find("button").exists()).toBe(false);
  });

  it("is told so even when the address is unknown", () => {
    const { wrapper } = when_shown([row({ port: 6667, web_url: null })]);

    expect(cellsOf(wrapper)[2].text()).toBe("not for the browser");
  });

  it("still can be opened on this computer", () => {
    const { wrapper } = when_shown([row({ port: 22, web_url: "http://finch.meshtown:22" })]);

    expect(wrapper.find('[data-port-open="ab/22"]').exists()).toBe(true);
  });
});

describe("the column for other programs", () => {
  it("has the number field and the \"Open on this computer\" button with its title", () => {
    const { wrapper } = when_shown([row({ port: 80 })]);

    const button = wrapper.get('[data-port-open="ab/80"]');
    expect(button.text()).toBe("Open on this computer");
    expect(button.attributes("title")).toBe(OPEN_TITLE);
    expect(wrapper.get('[data-port-local="ab/80"]').element.value).toBe("2080");
  });

  it("of a port opened here shows the local address, the \"open\" badge and \"Close\"", () => {
    const opened = { local: 2080, state: "listening", error: null };
    const { wrapper } = when_shown([row({ port: 80, opened })]);

    const text = wrapper.get('[data-port-row="ab/80"]').text();
    expect(text).toContain("127.0.0.1:2080CopyopenClose");
    expect(wrapper.find('[data-port-close="ab/80"]').exists()).toBe(true);
  });
});

describe("members with the same name", () => {
  it("get a title on the link and a note under the table", () => {
    const { wrapper } = when_shown(twins());

    const titles = wrapper.findAll("#ports-net-tbody a").map((a) => a.attributes("title"));
    expect(titles).toEqual([SAME_NAME_TITLE, SAME_NAME_TITLE]);
    expect(wrapper.text()).toContain(SAME_NAME_HINT);
  });

  it("are the only ones that get the title", () => {
    const { wrapper } = when_shown([...twins(), row({ node: "ef", node_name: "other", web_url: "http://other.meshtown" })]);

    const titles = wrapper.findAll("#ports-net-tbody a").map((a) => a.attributes("title"));
    expect(titles).toEqual([SAME_NAME_TITLE, SAME_NAME_TITLE, undefined]);
  });

  it("do not exist when every member has its own name: no title, no note", () => {
    const rows = [row({ web_url: URL_80 }), row({ node: "cd", node_name: "alice", web_url: "http://alice.meshtown" })];
    const { wrapper } = when_shown(rows);

    expect(wrapper.findAll("#ports-net-tbody a").every((a) => a.attributes("title") === undefined)).toBe(true);
    expect(wrapper.text()).not.toContain(SAME_NAME_HINT);
  });

  it("are not made by two ports of one member", () => {
    const { wrapper } = when_shown([row({ port: 80, web_url: URL_80 }), row({ port: 8080, web_url: URL_8080 })]);

    expect(wrapper.text()).not.toContain(SAME_NAME_HINT);
    expect(wrapper.find("#ports-net-tbody a").attributes("title")).toBeUndefined();
  });
});

describe("the hints under the table", () => {
  it("are three paragraphs: the browser, https errors, other programs", () => {
    const { wrapper } = when_shown([row({})]);

    const hints = wrapper.findAll(".card")[0].findAll("p.hint").slice(-3).map((p) => p.text());
    expect(hints).toHaveLength(3);
    expect(hints[0]).toBe("Click an address to open it in this browser. It works in a browser set up to use Mesh Town (see the Home tab). Type addresses with http:// at the start, or the browser may search the web instead.");
    expect(hints[1]).toContain("Mesh Town can't explain errors on https:// addresses");
    expect(hints[1]).toContain("http://home-openwrt.meshtown:443");
    expect(hints[2]).toContain("click “Open on this computer”");
    expect(hints[2]).toContain("(80 → 2080)");
  });
});

// Setup and helpers.

const row = (over) => ({ node: "ab", node_name: "finch", online: true, port: 80, name: "luci", published: true, opened: null, web_url: URL_80, ...over });

const twins = () => [
  row({ node: "ab", web_url: "http://finch-ab12.meshtown" }),
  row({ node: "cd", web_url: "http://finch-cd34.meshtown" }),
];

function when_shown(rows, { clipboard = "works" } = {}) {
  written = [];
  const writeText = clipboard === "works" ? async (s) => void written.push(s) : async () => { throw new Error("denied"); };
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  mounted = mountSection(PortsSection, {
    state: baseState({ ports: { published: [], network: rows } }),
    config: baseConfig({ ports: { published: [], opened: [] } }),
  });
  return mounted;
}

const headers = (wrapper) => wrapper.findAll("table[aria-label='Ports shared with you'] thead th").map((th) => th.text());
const cellsOf = (wrapper) => wrapper.get("#ports-net-tbody tr").findAll("td");
const copyButton = (wrapper, url) => wrapper.get("#ports-net-tbody").find(`button[aria-label="Copy address ${url}"]`);
