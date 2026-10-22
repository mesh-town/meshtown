import { test, expect } from '../fixtures/index';
import { SECTIONS } from '../pages/tabs-bar';

// With a `.topbar` header above the panel the first two steps
// would fail; with the current layout (no header, logo in #tabs) the test is green. Default 1280×800 screen, no resize.
test('40-top-panel: wide screen: #tabs starts at the top of the window and contains the network logo, there is no old header above the panel', async ({
  app,
}) => {
  const panel = app.topPanel;

  await test.step('#tabs starts at the very top of the window (y≈0)', async () => {
    const top = await panel.top(panel.tabList.first());
    expect.soft(top, '#tabs starts at the very top of the window (y≈0)').not.toBeNull();
    expect.soft(top ?? Number.POSITIVE_INFINITY, `y=${top}`).toBeLessThanOrEqual(1);
  });

  await test.step('network logo ([data-net-name]/.brand-name) inside #tabs', async () => {
    await expect
      .soft(panel.logo, 'inside #tabs there is neither [data-net-name] nor .brand-name')
      .not.toHaveCount(0);
  });

  for (const section of SECTIONS) {
    await test.step(`tab "${section}" switches (wide screen)`, async () => {
      expect.soft(await app.tabs.open(section), `tab "${section}" switches`).toBe(true);
    });
  }

  await test.step('the old separate .topbar header above the #tabs panel is gone', async () => {
    const { above, detail } = await panel.oldHeaderAbove();
    expect.soft(above, detail).toBe(false);
  });
});
