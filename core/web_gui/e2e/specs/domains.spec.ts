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
