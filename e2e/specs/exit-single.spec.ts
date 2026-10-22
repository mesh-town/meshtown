import { test, expect } from '../fixtures/index';
import { peerLabel } from '../pages/glossary';
import {
  POLL_TIMEOUT_MS,
  expectConfigExits,
  routeState,
  type StateDoc,
  type Peer,
} from '../helpers/route-exits-api';

test.skip(({ targetKind }) => targetKind === 'node', 'a single node, no member to act as an exit (like 17-peer-exit)');

test(
  '34-via-mesh-off-direct: clicking "Directly, without an exit" clears route.exits, the route goes to direct',
  async ({ app, api }) => {
    const exit = app.exitCard;
    const state = await api.getJson<StateDoc>('/api/state');
    const found = (state.body?.peers ?? []).find((p) => !p.self && p.exit === true);
    test.skip(found === undefined, 'no demo member with exit===true on the target');
    const peer = found as Peer;

    await test.step('clicking the P row picks it as the exit (route.state == exit)', async () => {
      await exit.choosePeer(peerLabel(peer, true, state.body?.peers ?? []));
      await expect
        .poll(() => routeState(api), { timeout: POLL_TIMEOUT_MS, message: 'route.state' })
        .toBe('exit');
    });

    await test.step('clicking #exit-direct: route.exits == [] on the node', async () => {
      await exit.chooseDirect();
      await expectConfigExits(api, []);
    });

    await test.step('/api/state: route.state == direct after choosing "as usual"', async () => {
      await expect.poll(() => routeState(api), { timeout: POLL_TIMEOUT_MS }).toBe('direct');
    });

    await test.step('P in /api/state.peers is still online and exit===true (we did not touch it)', async () => {
      const after = await api.getJson<StateDoc>('/api/state');
      const p = (after.body?.peers ?? []).find((x) => x.id_full === peer.id_full);
      expect.soft(p?.online, 'P online').toBe(true);
      expect.soft(p?.exit, 'P exit').toBe(true);
    });

    await test.step('#exit-direct is in place and checked', async () => {
      await expect.soft(exit.direct).toHaveCount(1);
      await expect.soft(exit.direct).toBeChecked();
    });

    await test.step('all input.exit-choice are available (not disabled), none checked', async () => {
      await expect.soft(exit.choices.first(), 'no candidate rows').toBeVisible();
      for (const box of await exit.choices.all()) {
        await expect.soft(box).toBeEnabled();
        await expect.soft(box).not.toBeChecked();
      }
    });
  },
);
