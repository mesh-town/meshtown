/* `ackNotice`: a refused acknowledgement keeps the notice, a 404 (the node already dropped it) removes it. */

import { beforeEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { api, ApiError } from "../core/api.ts";
import { createStore } from "../core/store.ts";
import { ackNotice } from "./use-notices.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const storeWithNotices = () => {
  const store = createStore();
  store.notices = [{ id: 1 }, { id: 2 }];
  return store;
};

describe("ackNotice", () => {
  beforeEach(() => api.mockReset());

  it("sends the id and drops the notice from the store", async () => {
    api.mockResolvedValue({});
    const store = storeWithNotices();
    assert.equal(await ackNotice(store, 1), true);
    assert.deepEqual(api.mock.calls[0], ["POST", "/api/notices/ack", { id: 1 }]);
    assert.deepEqual(store.notices.map((n) => n.id), [2]);
  });

  it("404 not_found: the notice is already gone, so it is dropped and true is returned", async () => {
    api.mockRejectedValue(new ApiError(404, "not_found"));
    const store = storeWithNotices();
    assert.equal(await ackNotice(store, 1), true);
    assert.deepEqual(store.notices.map((n) => n.id), [2]);
  });

  it("another ApiError keeps the notice and returns false", async () => {
    api.mockRejectedValue(new ApiError(500, "internal"));
    const store = storeWithNotices();
    assert.equal(await ackNotice(store, 1), false);
    assert.equal(store.notices.length, 2);
  });

  it("a non-ApiError failure keeps the notice and returns false", async () => {
    api.mockRejectedValue(new Error("network down"));
    const store = storeWithNotices();
    assert.equal(await ackNotice(store, 1), false);
    assert.equal(store.notices.length, 2);
  });
});
