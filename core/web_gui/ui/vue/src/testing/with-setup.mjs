/* `withSetup(fn)`: calls `fn` in the `setup` of a host component mounted without a DOM
 * (`createRenderer` with empty node operations), so that `onMounted`/`onUnmounted` work
 * and there are no Vue warnings about hooks outside a component. */

import { createRenderer, defineComponent, h } from "vue";

const nodeOps = {
  insert: (el, parent) => { parent.children = parent.children || []; parent.children.push(el); },
  remove: () => {},
  createElement: (tag) => ({ tag, children: [] }),
  createText: (text) => ({ text }),
  createComment: (text) => ({ text }),
  setText: () => {},
  setElementText: () => {},
  parentNode: () => null,
  nextSibling: () => null,
  patchProp: () => {},
};

/** → `{ result, unmount }`: `result` is what `fn` returned; `unmount` calls `onUnmounted`. */
export function withSetup(fn) {
  let result;
  const host = defineComponent({ setup() { result = fn(); return () => h("i"); } });
  const app = createRenderer(nodeOps).createApp(host);
  app.mount({ children: [] });
  return { result, unmount: () => app.unmount() };
}
