import { test, expect } from '../fixtures/index';

// A direct PUT /api/config with a revision that is already behind: the node answers 409 and hands back the current document,
// so the page can refetch it; nothing from the stale body reaches the file.

interface ConfigDoc {
  rev?: number;
  config?: { checks?: { provider_dns?: string | null } } & Record<string, unknown>;
}

interface StaleReply {
  error?: string;
  message?: string;
  rev?: number;
  config?: ConfigDoc['config'];
}

const FRESH_VALUE = '8.8.4.4:53';
const STALE_VALUE = '1.0.0.1:53';

test('save-409-stale-rev: PUT /api/config with an outdated rev: 409 "stale_rev" with the current rev and config; the file keeps the earlier save', async ({
  api,
}) => {
  const cfg0 = await api.getJson<ConfigDoc>('/api/config');
  expect(cfg0.status, 'GET /api/config').toBe(200);
  const staleRev = cfg0.body?.rev;
  expect(staleRev, 'the document carries a rev').toBeDefined();

  await test.step('given: another writer saved a change, so the rev we hold is behind', async () => {
    const fresh = structuredClone(cfg0.body?.config ?? {}) as NonNullable<ConfigDoc['config']>;
    fresh.checks = { ...fresh.checks, provider_dns: FRESH_VALUE };
    const saved = await api.putJson('/api/config', { rev: staleRev, config: fresh });
    expect(saved.status, `the first PUT: ${JSON.stringify(saved.body)}`).toBe(200);
  });

  const current = await api.getJson<ConfigDoc>('/api/config');
  expect(current.body?.rev ?? -1, 'the save moved the rev').toBeGreaterThan(staleRev ?? Number.MAX_SAFE_INTEGER);

  const stale = structuredClone(cfg0.body?.config ?? {}) as NonNullable<ConfigDoc['config']>;
  stale.checks = { ...stale.checks, provider_dns: STALE_VALUE };
  const reply = await api.putJson<StaleReply>('/api/config', { rev: staleRev, config: stale });

  await test.step('when the stale rev is sent: 409 with error "stale_rev"', async () => {
    expect.soft(reply.status, JSON.stringify(reply.body)).toBe(409);
    expect.soft(reply.body?.error).toBe('stale_rev');
    expect.soft(reply.body?.message ?? '', 'a human message is there').not.toBe('');
  });

  await test.step('the body carries the current rev and the current config, not the stale one', async () => {
    expect.soft(reply.body?.rev, 'rev in the 409 body').toBe(current.body?.rev);
    expect.soft(reply.body?.config?.checks?.provider_dns, 'config in the 409 body').toBe(FRESH_VALUE);
  });

  await test.step('nothing from the stale body was written', async () => {
    const after = await api.getJson<ConfigDoc>('/api/config');
    expect.soft(after.body?.rev, 'rev did not move').toBe(current.body?.rev);
    expect.soft(after.body?.config?.checks?.provider_dns).toBe(FRESH_VALUE);
  });
});
