import { test, expect } from '../fixtures/index';

interface StateDoc {
  network?: unknown;
}

interface ConfigDoc {
  config?: { network?: { masked?: string } };
}

test.describe('71-first-start', () => {
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('71-first-start: a node without a network — on "Home" a heading and two buttons, the forms appear on click, a snapshot without a network', async ({ app, api }) => {
    const net = app.netCard;

    await test.step('the heading "You haven\'t joined a Mesh Town network yet" and both buttons are visible, no input fields', async () => {
      await expect.soft(net.startTitle).toBeVisible();
      await expect.soft(net.startJoin).toBeVisible();
      await expect.soft(net.startCreate).toBeVisible();
      await expect.soft(app.joinCard.keyInput).toBeHidden();
      await expect.soft(net.createLabelInput).toBeHidden();
    });

    await test.step('click "Join with an invite" — the invite field is visible, the network name field is not', async () => {
      await net.startJoin.click();
      await expect.soft(app.joinCard.keyInput).toBeVisible();
      await expect.soft(net.createLabelInput).toBeHidden();
    });

    await test.step('click "Create a new network" — the name field is visible, the invite field is not', async () => {
      await net.startCreate.click();
      await expect.soft(net.createLabelInput).toBeVisible();
      await expect.soft(app.joinCard.keyInput).toBeHidden();
    });

    await test.step('API: GET /api/state — network: null', async () => {
      const state = await api.getJson<StateDoc>('/api/state');
      expect.soft(state.status).toBe(200);
      expect.soft(state.body?.network).toBeNull();
    });
  });

  test('71-first-start (file): a real node without a network has no network key in the config (config.network.masked is empty)', async ({ api, targetKind }) => {
    // DemoNode keeps a ready network label in the config: "a file without a key" is checked only by a real node
    test.skip(targetKind === 'demo', 'DemoNode does not tie config.network to the network state');

    const config = await api.getJson<ConfigDoc>('/api/config');
    expect.soft(config.status).toBe(200);
    expect.soft(config.body?.config?.network?.masked).toBe('');
  });
});
