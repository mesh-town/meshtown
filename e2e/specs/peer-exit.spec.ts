import { test, expect } from '../fixtures/index';
import { PeersSection, type PeerExitAction } from '../pages/peers-section';
import type { MeshApi } from '../fixtures/api';

interface Peer {
  id: string;
  id_full: string;
  name?: string | null;
  self?: boolean;
  online?: boolean;
  exit: boolean | null;
  version?: string | null;
}
interface StateDoc {
  peers?: Peer[];
}
interface RouteDoc {
  rev?: unknown;
  config?: { route?: { exits?: string[] } } & Record<string, unknown>;
}

const NODE_SKIP_17 = 'a single node, no other members';
const NODE_SKIP_44 = 'a single node, no other members to choose an exit from';

// The chosen exit is shown by the row with a "✓ …" mark even when offline (exit=null).
const exitAction = (value: boolean | null, chosen: boolean): PeerExitAction =>
  value === true || chosen ? 'yes' : value === false ? 'no' : '—';

/** How #route-state names a member: by its name, by the code when it has none. */
function routeLabelOf(p: Peer): string {
  return p.name?.trim() ? p.name : p.id;
}

async function foreignPeers(api: MeshApi): Promise<{ status: number; others: Peer[] }> {
  const { status, body } = await api.getJson<StateDoc>('/api/state');
  const peers = status === 200 ? (body?.peers ?? []) : [];
  return { status, others: peers.filter((p) => !p.self) };
}

async function selfPeer(api: MeshApi): Promise<Peer | undefined> {
  const { body } = await api.getJson<StateDoc>('/api/state');
  return (body?.peers ?? []).find((p) => p.self);
}

async function routeExitsOf(api: MeshApi): Promise<string[]> {
  const cfg = await api.getJson<RouteDoc>('/api/config');
  return cfg.body?.config?.route?.exits ?? [];
}

/** Annotates a step as not checked (a skip of a step inside a case) when the condition is true. */
function noteSkip(condition: boolean, description: string): void {
  test.info().annotations.push(...(condition ? [{ type: 'skip', description }] : []));
}

test('17-peer-exit: nodes of other members have exit — a text and a button in #peers-list; the exit choice is a radio button in #home-exit', async ({
  app,
  api,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', NODE_SKIP_17);
  const homeExit = app.exitCard;
  const peers = new PeersSection(page);

  await test.step('#exit-list: each row has a radio button to the left of the caption, left edges on one vertical line', async () => {
    await homeExit.expectRadiosAligned();
  });

  await app.tabs.open('peers');
  const { status, others } = await foreignPeers(api);
  const routeExits = await routeExitsOf(api);

  await test.step('API: peers.exit of other members ∈ {true,false,null}, offline — null', () => {
    expect.soft(status).toBe(200);
    expect.soft(others.length, 'no other members').toBeGreaterThan(0);
    for (const p of others) {
      expect.soft([true, false, null], `exit of ${p.id}`).toContain(p.exit);
    }
    for (const p of others.filter((o) => !o.online)) {
      expect.soft(p.exit, `offline ${p.id} must have exit=null`).toBeNull();
    }
  });

  await test.step('in #peers-list the rows of other members have no checkboxes', async () => {
    for (const p of others) {
      await expect.soft(peers.pickControls(peers.row(p.id))).toHaveCount(0);
    }
  });

  await test.step('in #peers-list the row action is "yes" (a button or a mark)/"no" (not allowed)/"—" by the exit value', async () => {
    await expect.soft(peers.items.first()).toBeVisible();
    for (const p of others) {
      await peers.expectAction(peers.row(p.id), exitAction(p.exit, routeExits.includes(p.id_full)), p.id);
    }
  });

  await test.step('in #peers-list the country is a code for peer-one (RU), "—" for peer-two', async () => {
    await peers.expectCountries({ 'peer-one': 'RU', 'peer-two': '—' });
  });

  await test.step('in #peers-list the version: "You" has its own without "older", peer-one has 0.0.9 and "older", peer-two (version=null) has neither a version nor a mark', async () => {
    const self = await selfPeer(api);
    const selfRow = peers.items.first();
    expect.soft(self?.version, 'the own row in the snapshot carries the build version').toMatch(/^\d+\.\d+\.\d+/);
    await expect.soft(selfRow.locator('.peer-version')).toHaveText(self?.version ?? '');
    await expect.soft(selfRow.locator('.peer-older'), '"You" has no mark').toHaveCount(0);

    await peers.expectVersions(self?.version ?? '');
  });

  await test.step('in #home-exit a radio button for exit=true or from route.exits, checked ⇔ in route.exits', async () => {
    const expectedIds = others
      .filter((p) => p.exit === true || routeExits.includes(p.id_full))
      .map((p) => p.id_full);
    noteSkip(
      expectedIds.length === 0,
      'not checked: this target has no other member with exit=true and route.exits is empty',
    );
    await app.tabs.open('home');
    for (const idFull of expectedIds) {
      await expect.soft(homeExit.choiceById(idFull), idFull).toBeChecked({ checked: routeExits.includes(idFull) });
    }
  });
});

test('44-peer-code-matches-route-state: .peer-code of a member in the "Members" list = the code of the same member in #route-state when it is chosen as the exit', async ({
  app,
  api,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', NODE_SKIP_44);
  const peers = new PeersSection(page);
  await app.tabs.open('peers');
  const { others } = await foreignPeers(api);
  const label = '#route-state contains the same code as .peer-code of the member chosen as the exit';
  test.skip(others.length === 0, 'not checked: this target has no other members');
  // Rows of other members follow the snapshot order: the first of them is others[0]; its code on the page is `p.id`.
  const tableCode = others[0].id;
  // #route-state names the member by its name (the code only for an unnamed member or a name shared with another one).
  const routeLabel = routeLabelOf(others[0]);
  await expect.soft(peers.row(tableCode), 'row of the first other member').toBeVisible();

  const cfg = await api.getJson<RouteDoc>('/api/config');
  expect(cfg.status, 'GET /api/config').toBe(200);
  const config = { ...cfg.body?.config, route: { ...cfg.body?.config?.route, exits: [others[0].id_full] } };
  const put = await api.putJson('/api/config', { rev: cfg.body?.rev, config });
  expect(put.status, 'PUT /api/config route.exits').toBe(200);

  await app.tabs.open('home');

  await test.step(label, async () => {
    await expect.soft(app.homePanel.routeState, `code=${tableCode}`).toContainText(routeLabel, { timeout: 5000 });
  });
});
