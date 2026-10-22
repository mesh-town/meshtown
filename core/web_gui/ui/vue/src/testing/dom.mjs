/* Element visibility in happy-dom, which does not compute styles: we look at the `hidden` attribute and
 * `style.display`. Shared by the `*.flows.vtest.mjs` cases moved over from e2e. */

// Visible: neither the element nor its ancestors are hidden (`hidden`, `display:none`).
export function shown(el) {
  for (let cur = el; cur && cur !== document.body; cur = cur.parentElement) {
    if (cur.hasAttribute("hidden") || cur.style.display === "none") return false;
  }
  return true;
}

// The element itself is hidden, or it is the content of a collapsed `<details>` (except `<summary>`), like `innerText`.
function hiddenSelf(el) {
  if (el.hasAttribute("hidden") || el.style.display === "none") return true;
  return el.parentElement?.tagName === "DETAILS" && !el.parentElement.open && el.tagName !== "SUMMARY";
}

// Stricter than `shown`: also takes collapsed `<details>` among the ancestors into account.
export function shownStrict(el) {
  for (let cur = el; cur && cur !== document.body; cur = cur.parentElement) {
    if (hiddenSelf(cur)) return false;
  }
  return true;
}

// Visible text, like `innerText`: without hidden and collapsed nodes.
export function visibleText(el) {
  if (hiddenSelf(el)) return "";
  return [...el.childNodes].map((n) => (n.nodeType === 3 ? n.textContent : n.nodeType === 1 ? visibleText(n) : "")).join(" ");
}
