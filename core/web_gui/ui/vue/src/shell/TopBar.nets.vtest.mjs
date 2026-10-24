/* The header with two networks: an exit in the second one is named together with its network. */

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { routeLine } from "../home/route-line.ts";
import { netsStore } from "../testing/nets-store.mjs";
import { createConn } from "./conn.ts";
import { statusLine } from "./status-line.ts";
import TopBar from "./TopBar.vue";

const Y = "E".repeat(64);

describe("TopBar with two networks", () => {
  it("the header says via the member and names network B", () => {
    const store = netsStore([
      { id: "main", label: "home", offers: [], exits: [], proxy: false },
      { id: "k7q2xa", label: "work", offers: [Y], exits: [Y], proxy: true },
    ]);
    const status = statusLine(store.state, createConn(), store.members, routeLine(store));
    const w = mount(TopBar, { props: { status } });
    expect(w.get("[role=status]").text()).toMatch(/^Sites open via exit-e \(work\) · home/);
    expect(w.get(".dot").classes()).toContain("dot-green");
  });
});
