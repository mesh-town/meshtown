import { test, expect } from '../fixtures/index';
import { SECTIONS } from '../pages/tabs-bar';

// This test has no `page`: the target's transport-level protection is checked with direct HTTP requests,
// no browser is needed for it.
test('12-guards: the target rejects a foreign Host, a foreign Origin and a request without a token', async ({
  api,
  target,
}) => {
  await test.step('valid token, valid Host — access', async () => {
    const code = await api.status({
      method: 'GET',
      path: `/api/state?token=${target.token}`,
      headers: { Host: target.host },
    });
    expect.soft(code, 'valid token, valid Host — access').toBe(200);
  });

  await test.step('no token — rejected', async () => {
    const code = await api.status({ method: 'GET', path: '/api/state' });
    expect.soft(code, 'no token — rejected').toBe(401);
  });

  await test.step('wrong token — rejected', async () => {
    const code = await api.status({ method: 'GET', path: '/api/state?token=nope' });
    expect.soft(code, 'wrong token — rejected').toBe(401);
  });

  await test.step('valid token, foreign Host — rejected', async () => {
    const code = await api.status({
      method: 'GET',
      path: `/api/state?token=${target.token}`,
      headers: { Host: 'evil.example' },
    });
    expect.soft(code, 'valid token, foreign Host — rejected').toBe(403);
  });

  await test.step('valid token, foreign Origin, PUT — rejected', async () => {
    const code = await api.status({
      method: 'PUT',
      path: `/api/config?token=${target.token}`,
      headers: { Origin: 'http://evil.example' },
      data: {},
    });
    expect.soft(code, 'valid token, foreign Origin, PUT — rejected').toBe(403);
  });

  await test.step('static files without a token — available', async () => {
    const code = await api.status({ method: 'GET', path: '/css/00-base.css' });
    expect.soft(code, 'static files without a token — available').toBe(200);
  });

  await test.step('token in the X-Meshtown-Token header, no token in the address — access', async () => {
    const code = await api.status({
      method: 'GET',
      path: '/api/state',
      headers: { 'X-Meshtown-Token': target.token },
    });
    expect.soft(code, 'token in the X-Meshtown-Token header, no token in the address — access').toBe(200);
  });
});

test('13-narrow: 390×844: none of the seven sections is cut off by width', async ({ app }) => {
  await app.resize({ width: 390, height: 844 });

  await test.step('section "home" is not wider than the screen', async () => {
    const w = await app.sectionWidth('home');
    expect.soft(w, 'section "home" is not wider than the screen').toBeLessThanOrEqual(390);
  });

  await test.step('#tabs — the strip is above the content (its box top is smaller than that of #sec-home)', async () => {
    const tabsTop = await app.top(app.tabs.root);
    const homeTop = await app.top(app.homePanel.root);
    expect.soft(tabsTop, '#tabs — the strip is above the content').toBeLessThan(homeTop);
  });

  for (const s of SECTIONS.slice(1)) {
    await test.step(`section "${s}" is not wider than the screen`, async () => {
      const w = await app.sectionWidth(s);
      expect.soft(w, `section "${s}" is not wider than the screen`).toBeLessThanOrEqual(390);
    });
  }
});

test('33-narrow-tabs-fit: 390×844: the tab strip fits without scrolling, no panel is cut off on the right', async ({
  app,
}) => {
  await app.resize({ width: 390, height: 844 });

  await test.step('#tabs does not scroll: scrollWidth <= clientWidth', async () => {
    const { scrollWidth, clientWidth } = await app.tabs.scrollMetrics();
    expect.soft(scrollWidth, '#tabs does not scroll: scrollWidth <= clientWidth').toBeLessThanOrEqual(clientWidth);
  });

  for (const s of SECTIONS) {
    const label = `panel "${s}" fits the width (390px)`;

    await test.step(label, async () => {
      expect.soft(await app.panelRight(s), label).toBeLessThanOrEqual(390);
    });
  }

  await test.step('#tab-settings is reachable: right edge ≤ 390 (1px tolerance for fractional pixels) after choosing "Settings"', async () => {
    const right = await app.right(app.tabs.tab('settings'));
    expect
      .soft(right, '#tab-settings is reachable: right edge ≤ 390 (1px tolerance for fractional pixels) after choosing "Settings"')
      .toBeLessThanOrEqual(391);
  });
});
