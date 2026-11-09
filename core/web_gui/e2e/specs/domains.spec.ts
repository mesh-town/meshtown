import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { DomainsSection } from '../pages/domains-section';

// A real node without domain fixtures starts empty (`build_demo_domains()` exists only in the rust demo);
// the check is pure CSS and does not depend on the target, one target is enough.
test.skip(({ targetKind }) => targetKind === 'node', 'a real node has no domain fixtures; the check is pure CSS');

test('46-domains-buttons-desktop-size: the "Recheck"/"Remove" buttons in the domains table are the size of a regular page button on desktop, not a mini button', async ({
  app,
  page,
}) => {
  await app.tabs.open('domains');
  const domains = new DomainsSection(page);
  const label = 'domain row button: same height as a regular button (#domains-check-all-btn), with a border';

  await test.step(label, async () => {
    // The domain row is drawn after polling; wait up to 5 s, otherwise fail with the label.
    await expect(domains.rowRecheckButton, label).toBeVisible({ timeout: 5000 });
    const ref = await domains.height(domains.checkAllButton);
    const got = await domains.height(domains.rowRecheckButton);
    const border = await domains.borderStyle(domains.rowRecheckButton);
    expect.soft(ref, `${label}: reference height`).toBeGreaterThan(0);
    expect.soft(Math.abs(got - ref), `${label}: ref=${ref}, got=${got}`).toBeLessThanOrEqual(3);
    expect.soft(border, `${label}: border=${border}`).not.toBe('none');
  });
});

test('72-domains-narrow-360: 360×800: the "Site address" field is not squeezed, "Add" is within the window, the sites are cards that fit the window (no table), every "Remove" is within the window', async ({
  app,
  page,
}) => {
  await app.resize({ width: 360, height: 800 });
  await app.tabs.open('domains');
  const domains = new DomainsSection(page);
  await expect(domains.cardRecheckButton, 'no site card: the section is not mounted').toBeVisible({ timeout: 10000 });

  expect.soft(await domains.pageScrollWidth(), 'the page is wider than the 360 window').toBeLessThanOrEqual(360);
  const inputWidth = await domains.width(domains.addInput);
  expect.soft(inputWidth, `the "Site address" field is squeezed: ${inputWidth}px`).toBeGreaterThan(200);
  const addRight = await domains.rightEdge(domains.addButton);
  expect.soft(addRight, `the "Add" button goes beyond the window: right edge ${addRight}`).toBeLessThanOrEqual(360);
  await expect.soft(page.getByRole('table'), 'a table on a narrow screen: the sites must be cards').toHaveCount(0);
  for (const remove of await domains.cardRemoveButtons.all()) {
    const right = await domains.rightEdge(remove);
    expect.soft(right, `a "Remove" button goes beyond the window: right edge ${right}`).toBeLessThanOrEqual(360);
  }
});

interface OverviewSlot {
  state?: { peers?: { id?: string }[]; domains?: { checks?: Record<string, unknown> }[] } | null;
}

// The demo starts with no checks; every member gets an "ok" verdict for every site, so a row shows "also via N more".
async function everySiteOpensEverywhere(page: Page): Promise<void> {
  const ok = { verdict: 'ok', reason: '', dns_ms: 10, tcp_ms: 10, tls_ms: 10, http_ms: 10, status: 200, bytes: 100, stalled: false, age_sec: 5 };
  await page.route('**/api/overview', async (route) => {
    const res = await route.fetch();
    const body = (await res.json()) as { slots?: OverviewSlot[] };
    for (const slot of body.slots ?? []) {
      const peers = slot.state?.peers ?? [];
      for (const domain of slot.state?.domains ?? []) {
        domain.checks = Object.fromEntries(peers.map((p) => [p.id ?? '', ok]));
      }
    }
    await route.fulfill({ response: res, json: body });
  });
}

const CENTER_SPREAD_MAX = 2;
// The height of the "Recheck"/"Remove" buttons before the glyphs shrank to 16 px (an 18 px glyph, padding and border).
const BUTTON_HEIGHT = 34;

test('133-domains-row-vertical-center: 1280: in a site row the name, the result mark, "N more", "Details", "Recheck" and "Remove" share one vertical center', async ({
  app,
  page,
}) => {
  await everySiteOpensEverywhere(page);
  await app.resize({ width: 1280, height: 800 });
  await app.tabs.open('domains');
  const row = page.locator('tr.domain-row').filter({ has: page.locator('.site-more') }).first();
  await expect(row, 'no site row with an "N more" button').toBeVisible({ timeout: 10000 });

  const parts: Record<string, ReturnType<typeof row.locator>> = {
    name: row.locator('td.mono'),
    mark: row.locator('.badge').first(),
    more: row.locator('.site-more'),
    details: row.locator('td.domain-actions .btn-text'),
    recheck: row.getByRole('button', { name: /^Recheck / }),
    remove: row.getByRole('button', { name: /^Remove / }),
  };
  const centers: Record<string, number> = {};
  for (const [key, loc] of Object.entries(parts)) {
    centers[key] = await loc.evaluate((el) => {
      const r = el.getBoundingClientRect();
      return r.top + r.height / 2;
    });
  }
  const values = Object.values(centers);
  const spread = Math.max(...values) - Math.min(...values);
  expect.soft(spread, `vertical centers diverge: ${JSON.stringify(centers)}`).toBeLessThanOrEqual(CENTER_SPREAD_MAX);
});

test('134-domains-row-icons-16: 1280: the glyphs of the row buttons are 16×16 and the buttons keep their height', async ({ app, page }) => {
  await app.resize({ width: 1280, height: 800 });
  await app.tabs.open('domains');
  const row = page.locator('tr.domain-row').first();
  await expect(row, 'no site row').toBeVisible({ timeout: 10000 });

  for (const name of [/^Recheck /, /^Remove /]) {
    const button = row.getByRole('button', { name });
    const box = await button.boundingBox();
    expect.soft(Math.abs((box?.height ?? 0) - BUTTON_HEIGHT), `${name}: button height ${box?.height}`).toBeLessThanOrEqual(0.5);
    const svg = await button.locator('svg').boundingBox();
    expect.soft([svg?.width, svg?.height], `${name}: glyph size`).toEqual([16, 16]);
  }
});
