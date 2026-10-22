import { test, expect } from '../fixtures/index';
import { GLOSSARY } from '../pages/glossary';
import { STRANGER_ID, putRouteExits, type Peer } from '../helpers/route-exits-api';

const TEXT_TIMEOUT_MS = 10000;

test.describe('65-shell-status-line', () => {
  test.skip(({ targetKind }) => targetKind !== 'demo', 'members with an exit are needed: only the demo provides them');

  test('65-shell-status-line: header status line: exit chosen — "Sites open via <name>" with a green dot; an unknown exit — red dot; the same on "Log" and "Settings"', async ({
    app,
    api,
  }) => {
    const top = app.topPanel;
    const { body } = await api.getJson<{ peers?: Peer[] }>('/api/state');
    const exit = (body?.peers ?? []).find((p) => !p.self && p.exit && p.online);
    test.skip(!exit, 'no member with an exit in the demo snapshot');

    try {
      await test.step('exit chosen — "Sites open via <name> · <network> · N members online" and a green dot', async () => {
        const put = await putRouteExits(api, [exit!.id_full]);
        expect(put.status, `PUT body=${JSON.stringify(put.body)}`).toBe(200);
        await expect(top.statusLine).toHaveText(new RegExp('^' + GLOSSARY.routeLinePrefix + 'via [^(]+ · '), { timeout: TEXT_TIMEOUT_MS });
        await expect(top.statusLine).not.toContainText('In the browser');
        await expect(top.statusLine).not.toContainText(' more ');
        // no anchor: the dot is decoration (`aria-hidden`), it has no role or label
        await expect(top.statusLine.locator('.dot')).toHaveClass(/dot-green/);
      });

      await test.step('on "Log" and "Settings" the line is the same', async () => {
        const text = ((await top.statusLine.textContent()) ?? '').trim();
        for (const section of ['log', 'settings'] as const) {
          expect(await app.tabs.open(section), `tab "${section}" opened`).toBe(true);
          await expect(top.statusLine, `line on "${section}"`).toHaveText(text);
        }
      });

      await test.step('exit is an unknown id: red dot and "offline"', async () => {
        const put = await putRouteExits(api, [STRANGER_ID]);
        expect(put.status, `PUT body=${JSON.stringify(put.body)}`).toBe(200);
        await expect(top.statusLine).toContainText('offline', { timeout: TEXT_TIMEOUT_MS });
        await expect(top.statusLine.locator('.dot')).toHaveClass(/dot-red/);
      });
    } finally {
      await putRouteExits(api, []);
    }
  });
});
