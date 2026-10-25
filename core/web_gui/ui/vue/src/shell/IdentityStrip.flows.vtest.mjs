/* The identity strip on the whole page (DOM cases of the identity strip): the state
 * `node.identity` / `identity_reason` and the reason from `file.message` come from the mock node. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

const STRIP_TAIL = "After a restart, other members will see this computer as a new member.";
const MESSAGES = {
  declined: "The node identity isn't saved: this was set at startup",
  file_problem: "Couldn't read the settings file: access denied",
  file_preserved: "The settings file wasn't updated: the directory is read-only",
};

afterEach(unmountPage);

function nodeWith(identity, reason) {
  const base = demoState();
  const state = demoState({ node: { ...base.node, identity, identity_reason: reason } });
  const file = { path: "/tmp/config.json", state: "ok", writable: true, message: MESSAGES[reason] ?? null, field_errors: [] };
  const config = { rev: 1, config: demoConfig(), read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [], file };
  return createFakeNode({ state, config: demoConfig(), routes: { "GET /api/config": config } });
}

const hidden = (el) => el.hasAttribute("hidden") || el.style.display === "none";

describe("DOM", () => {
  it("DOM --identity persistent — the identity strip is hidden", async () => {
    const { wrapper } = await mountPage(nodeWith("persistent", null));
    expect(hidden(wrapper.get("#identity-strip").element), "the strip is visible with a saved identity").toBe(true);
  });

  const classes = { declined: "identity-strip-declined", file_problem: "identity-strip-problem", file_preserved: "identity-strip-preserved" };
  for (const [reason, cls] of Object.entries(classes)) {
    it(`DOM --identity ${reason} — the identity strip on the page`, async () => {
      const { wrapper } = await mountPage(nodeWith("temporary", reason));
      const strip = wrapper.get("#identity-strip");
      const msg = MESSAGES[reason].toLowerCase().slice(0, 24);
      expect(hidden(strip.element), "hidden although the identity is temporary").toBe(false);
      expect(strip.classes()).toContain(cls);
      expect(strip.text().endsWith(STRIP_TAIL)).toBe(true);
      expect(strip.text().toLowerCase(), "the reason is not from the node").toContain(msg);
    });
  }
});
