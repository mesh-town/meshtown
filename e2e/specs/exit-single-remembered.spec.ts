import { test, expect } from '../fixtures/index';
import {
  POLL_TIMEOUT_MS,
  STRANGER_ID,
  STRANGER_LABEL,
  expectConfigExits,
  putRouteExits,
} from '../helpers/route-exits-api';
import { AppShell } from '../pages/app-shell';

// The exit list lives only in a network (the Home setup without a network does not show it): the `--no-network` flag is not needed.

test(
  '35-exit-off-keeps-list: choosing "as usual" does not hide the exit list behind the "No other member…" text: the remembered node stays an available row',
  async ({ page, target, api, consoleLog }) => {
    void consoleLog;
    const put = await putRouteExits(api, [STRANGER_ID]);
    test.skip(put.status !== 200, `could not set route.exits before loading the page (status=${put.status})`);
    // Without exits offered by members the list holds only the remembered node: otherwise the "No other member…" check is empty.
    await page.route('**/api/state', async (route) => {
      const res = await route.fetch();
      const body = await res.json();
      body.peers = (body.peers || []).map((p: Record<string, unknown>) => ({ ...p, exit: false }));
      await route.fulfill({ response: res, json: body });
    });
    const app = new AppShell(page);
    await app.open(target.origin, target.token);
    const exit = app.exitCard;

    await test.step('clicked #exit-direct — route.exits == [] on the node', async () => {
      await exit.chooseDirect();
      await expectConfigExits(api, []);
    });

    await test.step(
      'after choosing "as usual": #exit-direct is checked, the remembered node row is in place (not disabled, not checked), no "No other member…"',
      async () => {
        await expect.soft(exit.direct).toBeChecked();
        const row = exit.choice(STRANGER_LABEL);
        await expect.soft(row).toHaveCount(1);
        await expect.soft(row).toBeEnabled();
        await expect.soft(row).not.toBeChecked();
        await expect.soft(exit.emptyHint).toHaveCount(0);
      },
    );

    await test.step('click on the remembered node row — route.exits == [P] (the choice itself is not restored)', async () => {
      await exit.choosePeer(STRANGER_LABEL);
      await expectConfigExits(api, [STRANGER_ID]);
    });

    await test.step('click on #exit-direct again — route.exits == [], the node row stays', async () => {
      await exit.chooseDirect();
      await expectConfigExits(api, []);
    });

    await test.step('#exit-direct is checked, the remembered node row is in place and not checked', async () => {
      await expect.soft(exit.direct).toBeChecked();
      await expect.soft(exit.choice(STRANGER_LABEL)).toHaveCount(1);
      await expect.soft(exit.choice(STRANGER_LABEL)).not.toBeChecked();
    });
  },
);

test(
  '36-exit-external-write-shows-on: a route.exits write bypassing the page (tray) — the radio buttons catch up by polling',
  async ({ page, target, api, consoleLog }) => {
    void consoleLog;
    const put = await putRouteExits(api, [STRANGER_ID]);
    test.skip(put.status !== 200, `could not set route.exits before loading the page (status=${put.status})`);
    const app = new AppShell(page);
    await app.open(target.origin, target.token);
    const exit = app.exitCard;

    await test.step('chose "as usual" on the page — route.exits == [] on the node', async () => {
      await exit.chooseDirect();
      await expectConfigExits(api, []);
    });

    await test.step('the external route.exits == [P] write went through', async () => {
      const res = await putRouteExits(api, [STRANGER_ID]);
      expect(res.status, `body=${JSON.stringify(res.body)}`).toBe(200);
    });

    await test.step('within ≤ 6 s of polling: #exit-direct is not checked, the P row is checked and not disabled', async () => {
      await expect(exit.direct).not.toBeChecked({ timeout: POLL_TIMEOUT_MS });
      const row = exit.choice(STRANGER_LABEL);
      await expect(row).toBeChecked({ timeout: POLL_TIMEOUT_MS });
      await expect(row).toBeEnabled({ timeout: POLL_TIMEOUT_MS });
    });
  },
);
