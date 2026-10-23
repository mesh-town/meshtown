# e2e — Playwright Test

Browser test suite for the node page, built on [Playwright Test](https://playwright.dev/); `make web-check` runs it.

## Running

From the repository root:

- `make web-check` — the `demo` target (the example server `core/examples/web_demo/`); builds it itself
  (cargo), 4 workers.
- `make web-check-node` — the `node` target (the real `meshtown`); builds it itself, 1 worker (its own target
  per test).
- `make web-check-nobuild` / `make web-check-node-nobuild` — the same sets without `cargo`: they take a prebuilt
  binary from `E2E_DEMO_BIN` / `E2E_NODE_BIN` (by default `$(TARGET_DIR)/debug/examples/web_demo` and
  `$(TARGET_DIR)/debug/meshtown`). If the file is missing, the target prints the path and a hint, exit code 2. For
  those who run already built binaries while other builds are going on.
- `make web-check-lint` — `tsc --noEmit` + ESLint (`--max-warnings 0`).
- `ONLY="1-basic 21"` — selects cases: numbers or names separated by spaces; the `case-` prefix is stripped.
- `JOBS=N` — number of workers (4 by default for `demo`, 1 for `node`).
- `HEADED=1` — a visible browser instead of the default hidden one (for manual runs: `WEB_E2E_HEADED=1`).
- One target, without `make`: `E2E_DEMO_BIN=<path> node_modules/.bin/playwright test --project=demo`
  in this directory (for `node` — `E2E_NODE_BIN` and `--project=node`).

`ONLY=` expands into `--grep`, which is matched against the string "project file describe test";
the pattern has a `[0-9]+-[a-z]` check built in, so a match is possible only at the start of a test name, and
not anywhere in the text. `ONLY=1` — only `1-basic` (not `21-…` and not "describe … 1"); `ONLY=4` —
both `4-file-invalid` and `4-file-unreadable` (both have number 4 — that is not a collision; to get just one,
use `ONLY=4-file-invalid`); `ONLY=15-invite` — both tests numbered 15.

The runner is `node_modules/.bin/playwright` (not `npx`). The first run installs Chromium (`install
chromium`, without `--with-deps`), which takes a while; `make` does this itself once.

Two runs in the same tree at the same time are not supported: they share one `outputDir`. Parallel sessions
work in their own trees, and the tree tag in names outside the tree keeps them apart; for several runs
in one tree, set different `E2E_RUN_TAG` values.

## Report and traces

Results, the HTML report, and failure traces are in `.tmp/e2e/` at the tree root (`results/`, `report/`), not in
`core/web_gui/e2e/`. For passed tests, `preserveOutput: 'failures-only'` deletes them.

## Layout

```
fixtures/  target launch, HTTP API, console — the shared test is assembled in fixtures/index.ts
            (the `fixtures/*.test.ts` tests do not depend on the target and run only in the demo project)
helpers/   helper API calls for specs (route-exits-api.ts)
pages/     page objects by page section — locators and actions, selectors only here
  app-shell.ts        the whole page: loading, window size, section widths and edges, card objects
  tabs-bar.ts         the tab strip
  top-panel.ts        the top strip: logo, network name, status line
  glossary.ts         dictionary of page texts, member label
  join-card.ts        the join card (JS moves it between Home and "Settings → Network")
  home/               Home: section (panel, card frames, first screen), net-card (network,
                      invite, members), nets-block (network choice), exit-card ("Exit via…"), name-card
                      (computer name, exit switch), proxy-card ("Connect the browser to Mesh Town")
  peers-section.ts, members-section.ts, ports-section.ts, network-section.ts, settings-section.ts,
  settings-file-banner.ts, domains-section.ts, log-section.ts   — one per tab
  ready.ts            waiting until the page and the node state are ready
specs/     specs: a file per section or feature; the test name carries the case number and slug
```

## How to add a case

0. First ask whether a browser is needed: a case that needs no binary, HTTP response, layout (geometry,
   computed CSS, "Back" history) or server-side logic is written not here but as `*.flows.vtest.mjs` next to the
   `.vue` file in `core/web_gui/ui/vue/src/` (vitest, the fake node `vue/src/testing/fake-node.mjs`); the static
   size check is `vue/src/static-limits.test.mjs` (vitest, node project). Only what cannot be checked without a browser goes here.
1. Take the next free number: `rg -o "^test\('[0-9]+-" specs` shows the taken ones. The test name is
   `N-slug: what is checked`; the spec is `specs/<section>.spec.ts` (a new file if the section does not exist).
2. Selectors live only in the page object of their section (`pages/`), never in the spec; name a method
   after the action or the property ("restart button is visible"), not after the markup.
3. Locators must survive a markup change. Order of anchors:
   `getByRole` with a name, `getByLabel`, `getByText`; `id` — only if the markup has no other anchor,
   and then a comment above the locator `// no anchor: <what the markup lacks>`; CSS classes,
   `nth-child`, and nesting chains are not used. Example: the "Restart Mesh Town"
   button is found by role and name, the message by text; the save bar and form fields in
   `settings-section.ts` rely on `id` because the markup gives them no role or label.
4. Waiting is only `expect(...)`, which waits by itself, and `expect.poll`; there is no `waitForTimeout` and
   no manual polling loops. No JS in strings; `getComputedStyle` in `evaluate` on an already
   found element is a measurement, not a selector.
5. A case that does not suit the `node` target — `test.skip(({ targetKind }) => targetKind === 'node', '<reason>')`
   in the spec.
6. Target flags for a test — `test.use({ targetFlags: { seed, fast, extraArgs } })` (the type is in
   `fixtures/target.ts`), not command-line arguments directly: the fixture assembles from them
   the argv suitable for the target kind.
7. Check: `make web-check-lint`, then `make web-check-nobuild ONLY=<number>` and the same for
   `web-check-node-nobuild`.

The demo target token carries the tree tag (8 hex of the `sha256` of the worktree root path) — names outside the tree do not
collide between parallel trees.
