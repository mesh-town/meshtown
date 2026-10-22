import { test, expect, type ConfigDoc } from '../fixtures/index';

const NEW_NAME = 'webcheck-home-name';

test('21-home-name-autosave: the name on the first screen is saved on Enter', async ({ app, api }) => {
  await test.step('the field status "Saved" within ≤ 3 s after Enter', async () => {
    await app.nameCard.rename(NEW_NAME);
    await expect.soft(app.nameCard.nameStatus).toHaveText('Saved', { timeout: 4000 });
  });

  await test.step('GET /api/config returns the new name', async () => {
    const reply = await api.getJson<ConfigDoc>('/api/config');
    expect.soft(reply.status).toBe(200);
    expect.soft(reply.body?.config?.node?.name).toBe(NEW_NAME);
  });
});

test.describe('22-home-name-blank', () => {
  test('22-home-name-blank: an empty name on the first screen is replaced with a random one', async ({ app, api }) => {
    const name = app.nameCard;
    const before = await api.getJson<ConfigDoc>('/api/config');
    const name0 = before.body?.config?.node?.name;

    // The page fills the field with the name from the node state while the config has null. Clearing
    // an empty field changes nothing (`saveHomeName`: value == the original) — wait for it to fill.
    await expect(name.nameField).not.toHaveValue('', { timeout: 4000 });
    await name.clearAndBlur();
    await expect.soft(name.nameField).not.toHaveValue('', { timeout: 4000 });

    await test.step('GET /api/config returns a new two-word name, not the previous one', async () => {
      await expect
        .poll(async () => (await api.getJson<ConfigDoc>('/api/config')).body?.config?.node?.name, { timeout: 4000 })
        .not.toBe(name0);
      const reply = await api.getJson<ConfigDoc>('/api/config');
      const name1 = reply.body?.config?.node?.name;
      expect.soft(reply.status).toBe(200);
      expect.soft(name1?.split('-').length, `was=${name0}, now=${name1}`).toBe(2);
    });

    await test.step('the field shows the new name', async () => {
      const name1 = (await api.getJson<ConfigDoc>('/api/config')).body?.config?.node?.name ?? '';
      await expect.soft(name.nameField).toHaveValue(name1);
    });
  });
});
