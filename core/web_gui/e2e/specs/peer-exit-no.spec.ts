import { test, expect } from '../fixtures/index';
import { PeersSection } from '../pages/peers-section';
import { GLOSSARY } from '../pages/glossary';

interface Peer {
  id: string;
  id_full: string;
  name?: string | null;
  self?: boolean;
}

test.describe('peer-exit-no', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a member without an exit is a flag of the demo node; skipped before the target starts');
  test.use({ targetFlags: { extraArgs: ['--no-exit-peers=1'] } });

  test('17b-peer-exit-no: a member that offers no exit has "Not available as an exit" in the list and no radio button in #home-exit', async ({ app, api, page }) => {
    const { body } = await api.getJson<{ peers?: Peer[] }>('/api/state');
    const others = (body?.peers ?? []).filter((p) => !p.self);
    const one = others.find((p) => p.name === 'peer-one');
    const two = others.find((p) => p.name === 'peer-two');
    expect(one, 'the demo has peer-one').toBeTruthy();
    expect(two, 'the demo has peer-two').toBeTruthy();
    const peers = new PeersSection(page);

    await app.tabs.open('peers');
    await expect(peers.row(two!.id)).toContainText(GLOSSARY.exitRefused);
    await peers.expectAction(peers.row(two!.id), 'no', 'peer-two');
    await peers.expectAction(peers.row(one!.id), 'yes', 'peer-one');

    await app.tabs.open('home');
    await expect(app.exitCard.choiceById(one!.id_full)).toHaveCount(1);
    await expect(app.exitCard.choiceById(two!.id_full)).toHaveCount(0);
  });
});
