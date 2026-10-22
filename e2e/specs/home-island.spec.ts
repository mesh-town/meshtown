import { test, expect } from '../fixtures/index';
import { GLOSSARY } from '../pages/glossary';
import { STRANGER_ID, STRANGER_LABEL, expectConfigExits, putRouteExits } from '../helpers/route-exits-api';
import type { Peer, StateDoc } from '../helpers/route-exits-api';

const DOM_TIMEOUT_MS = 6000;

test('62-home-state-panel: the status panel — "via a member", "offline", "Open sites as usual" restores the direct exit', async ({ app, api, targetKind }) => {
  test.skip(targetKind === 'node', 'a single node, no other members to choose an exit from');
  const state = await api.getJson<StateDoc>('/api/state');
  const other = (state.body?.peers ?? []).find((p: Peer) => !p.self && p.exit === true);
  test.skip(!other, 'not checked: this target has no member that allows being an exit');
  const panel = app.homePanel;

  await test.step('an exit is chosen — "In the browser, sites open via …"', async () => {
    await app.exitCard.choiceById(other?.id_full ?? '').click();
    await expect.soft(panel.routeState).toContainText(GLOSSARY.routeBrowserPrefix + 'via', { timeout: DOM_TIMEOUT_MS });
  });

  await test.step('the exit is offline — "… is offline — sites may not open or may open directly"', async () => {
    const put = await putRouteExits(api, [STRANGER_ID]);
    expect(put.status, 'PUT route.exits').toBe(200);
    await expect.soft(panel.routeState).toHaveText(`${STRANGER_LABEL} is offline — sites may not open or may open directly`, { timeout: DOM_TIMEOUT_MS });
  });

  await test.step('"Open sites as usual" — the text "directly", route.exits == []', async () => {
    await panel.statePanel.getByRole('button', { name: 'Open sites as usual' }).click();
    await expect.soft(panel.routeState).toContainText('directly', { timeout: DOM_TIMEOUT_MS });
    await expectConfigExits(api, []);
  });
});
