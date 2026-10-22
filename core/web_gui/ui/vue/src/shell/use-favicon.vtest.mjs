/* The tab icon hook: sets link[rel=icon] when the composition changes, not on every poll. */

import { afterEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { mount } from "@vue/test-utils";
import { createStore } from "../core/store.ts";
import { baseState } from "../testing/section.mjs";
import { compositionKey, useFavicon } from "./use-favicon.ts";

const href = () => document.querySelector("link[rel~='icon']")?.getAttribute("href");

function run(store) {
  return mount({ render: () => null, setup() { useFavicon(store); } });
}

afterEach(() => { document.head.querySelectorAll("link[rel~='icon']").forEach((l) => l.remove()); });

describe("useFavicon", () => {
  it("no link — creates it and sets data:image/svg+xml", () => {
    const store = createStore();
    store.state = baseState({});
    run(store);
    expect(href()).toMatch(/^data:image\/svg\+xml,/);
  });

  it("a link exists — edits it, does not create a second one", () => {
    document.head.insertAdjacentHTML("beforeend", '<link rel="icon" href="data:,">');
    run(createStore());
    expect(document.querySelectorAll("link[rel~='icon']")).toHaveLength(1);
    expect(href()).toMatch(/^data:image\/svg\+xml,/);
  });

  it("composition changed — new href; same composition — untouched", async () => {
    const store = createStore();
    store.state = baseState({});
    run(store);
    const before = href();
    document.querySelector("link[rel~='icon']").setAttribute("href", "marker");
    store.state = baseState({});
    await nextTick();
    expect(href()).toBe("marker");
    store.state = baseState({ peers: [{ id: "a", id_full: "a_full", name: "a", online: true, self: false }] });
    await nextTick();
    expect(href()).not.toBe("marker");
    expect(href()).not.toBe(before);
  });

  it("compositionKey counts roles", () => {
    expect(compositionKey([{ role: "online" }, { role: "online" }, { role: "via" }])).toBe("via1,client0,online2,offline0");
  });
});
