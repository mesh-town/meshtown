/* `useRefusalGate`: a refusal the user has already seen is hidden after a new attempt until the snapshot changes it. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ref } from "vue";
import { useRefusalGate } from "./use-refusal-gate.ts";

const refused = (over = {}) => ({ state: "refused", message: "no", label: "w", expires_unix: 1, ...over });
const waiting = () => ({ state: "waiting", message: "wait", label: "w", expires_unix: 1 });

describe("useRefusalGate", () => {
  it("passes the source through before any attempt", () => {
    const src = ref(refused());
    assert.deepEqual(useRefusalGate(() => src.value).info.value, refused());
  });
  it("dismiss hides the shown refusal", () => {
    const src = ref(refused());
    const gate = useRefusalGate(() => src.value);
    gate.dismiss();
    assert.equal(gate.info.value, null);
  });
  it("another refusal shows through", () => {
    const src = ref(refused());
    const gate = useRefusalGate(() => src.value);
    gate.dismiss();
    src.value = refused({ message: "other" });
    assert.equal(gate.info.value.message, "other");
  });
  it("the same refusal after waiting shows again", () => {
    const src = ref(refused());
    const gate = useRefusalGate(() => src.value);
    gate.dismiss();
    src.value = waiting();
    assert.equal(gate.info.value.state, "waiting");
    src.value = refused();
    assert.equal(gate.info.value.state, "refused");
  });
  it("dismiss without a refusal remembers nothing", () => {
    const src = ref(null);
    const gate = useRefusalGate(() => src.value);
    gate.dismiss();
    src.value = refused();
    assert.equal(gate.info.value.state, "refused");
  });
});
