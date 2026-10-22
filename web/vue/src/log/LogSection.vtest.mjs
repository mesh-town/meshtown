import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { navigateTo } from "../common/navigate.ts";
import { setToken } from "../core/token.ts";
import { mountSection } from "../testing/section.mjs";
import LogSection from "./LogSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));
vi.mock("../common/navigate.ts", () => ({ navigateTo: vi.fn() }));

const REPLY = { lines: [{ seq: 1, t: 0.5, tag: "net", text: "first" }, { seq: 2, t: 1.5, tag: "web", text: "second" }] };
let mounted;

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
});

const mountLog = async (detail) => {
  mounted = mountSection(LogSection, detail);
  await flushPromises();
  return mounted;
};

describe("LogSection", () => {
  it("the Log tab is open — response lines in role=log", async () => {
    api.mockResolvedValue(REPLY);
    const { wrapper } = await mountLog({ activeSection: "log" });
    expect(api).toHaveBeenCalledWith("GET", expect.stringContaining("/api/log"));
    const lines = wrapper.get("[role=log]").findAll(".log-line");
    expect(lines.map((l) => l.text())).toEqual([expect.stringContaining("first"), expect.stringContaining("second")]);
  });

  it("\"Pause\" switches to \"Resume\" and back", async () => {
    api.mockResolvedValue(REPLY);
    const { wrapper } = await mountLog({ activeSection: "log" });
    const btn = wrapper.get("#log-pause-btn");
    expect(btn.text()).toBe("Pause");
    await btn.trigger("click");
    expect(btn.text()).toBe("Resume");
    await btn.trigger("click");
    expect(btn.text()).toBe("Pause");
  });

  it("another section — api is not called, neither at once nor on the timer", async () => {
    const { wrapper } = await mountLog({ activeSection: "home" });
    await vi.advanceTimersByTimeAsync(3000);
    expect(api).not.toHaveBeenCalled();
    expect(wrapper.find("[role=log]").exists()).toBe(true);
  });

  it("\"Download .log\" goes to the download address with the token via navigateTo", async () => {
    api.mockResolvedValue(REPLY);
    setToken("tok en");
    const { wrapper } = await mountLog({ activeSection: "log" });
    await wrapper.get("#log-download-btn").trigger("click");
    expect(navigateTo).toHaveBeenCalledTimes(1);
    expect(navigateTo).toHaveBeenCalledWith("/api/log/download?token=tok%20en");
  });

  it("no state yet — \"Loading log…\"", async () => {
    const { wrapper } = await mountLog({ activeSection: "log", state: null });
    expect(wrapper.text()).toContain("Loading log…");
  });

  it("a non-network polling failure — the line \"Couldn't update the log: …\", a network one — silently", async () => {
    api.mockRejectedValue(Object.assign(new Error("Node is not responding"), { status: 0 }));
    const { wrapper } = await mountLog({ activeSection: "log" });
    expect(wrapper.find("#log-poll-error").exists()).toBe(false);
    api.mockRejectedValue(Object.assign(new Error("Node responded with an error (code 500)"), { status: 500 }));
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.get("#log-poll-error").text()).toBe("Couldn't update the log: node responded with an error (code 500)");
  });
});
