/* `members-texts.ts` for managed networks.
 *
 * Contract fixed by these tests:
 *  - `madeAdminStatus(name, net)`: the line under the list after "Make admin" — `{name} is now an admin of "{net}"`;
 *      `net` is the network name without quotes.
 *  - The menu labels and dialog texts of a managed network are fixed in `members-rows.managed.test.mjs` and
 *      `dialog-specs.managed.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { madeAdminStatus } from "./members-texts.ts";

describe("madeAdminStatus", () => {
  it("names the member and the quoted network", () => {
    assert.equal(madeAdminStatus("boris", "work"), 'boris is now an admin of "work"');
  });

  it("takes any name as shown in the list", () => {
    assert.equal(madeAdminStatus("member bbbb2222", "family"), 'member bbbb2222 is now an admin of "family"');
  });
});
