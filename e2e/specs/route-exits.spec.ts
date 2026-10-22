import { test, expect } from '../fixtures/index';
import {
  STRANGER_ID,
  STRANGER_LABEL,
  putRouteExits,
  routeState,
  type ConfigDoc,
  type StateDoc,
} from '../helpers/route-exits-api';
import { DomainsSection } from '../pages/domains-section';

// The /api/state snapshot updates asynchronously after PUT /api/config — poll instead of pausing.
const STATE_TIMEOUT_MS = 10000;
const DOM_TIMEOUT_MS = 6000;

interface ValidationBody {
  fields?: { path?: string }[];
}

test(
  '18-route-exits: route.exits in the config: the default value, PUT of a valid/empty/bad id, the snapshot and the page reflect the decision',
  async ({ app, api, page }) => {
    await test.step('GET /api/config: route.exits == [] by default, no all_traffic key', async () => {
      const { status, body } = await api.getJson<ConfigDoc>('/api/config');
      const route = body?.config?.route ?? {};
      expect.soft(status).toBe(200);
      expect.soft(route.exits, `route=${JSON.stringify(route)}`).toEqual([]);
      expect.soft('all_traffic' in route, 'all_traffic key').toBe(false);
    });

    await test.step('PUT [unknown id] -> 200, /api/state: route.state=unavailable, exits[0].online=false', async () => {
      const put = await putRouteExits(api, [STRANGER_ID]);
      expect(put.status, `PUT body=${JSON.stringify(put.body)}`).toBe(200);
      await expect
        .poll(
          async () => {
            const { status, body } = await api.getJson<StateDoc & { route?: { exits?: { online?: boolean }[] } }>(
              '/api/state',
            );
            const route = body?.route;
            return status === 200 && route?.state === 'unavailable' && route.exits?.length === 1
              ? route.exits[0].online
              : undefined;
          },
          { timeout: STATE_TIMEOUT_MS },
        )
        .toBe(false);
    });

    await test.step('PUT [] -> 200, /api/state: route.state=direct', async () => {
      const put = await putRouteExits(api, []);
      expect(put.status, `PUT body=${JSON.stringify(put.body)}`).toBe(200);
      await expect
        .poll(
          async () => {
            const { body } = await api.getJson<{ route?: { state?: string; exits?: unknown[] } }>('/api/state');
            return `${body?.route?.state}:${JSON.stringify(body?.route?.exits)}`;
          },
          { timeout: STATE_TIMEOUT_MS },
        )
        .toBe('direct:[]');
    });

    await test.step('PUT ["bad"] -> 422 with the route.exits field', async () => {
      const put = await putRouteExits(api, ['bad']);
      const fields = (put.body as ValidationBody | null)?.fields ?? [];
      expect(put.status, `body=${JSON.stringify(put.body)}`).toBe(422);
      expect(fields.map((f) => f.path)).toContain('route.exits');
    });

    // 422 is not applied — put unavailable back explicitly so the DOM check does not depend on the step order.
    const back = await putRouteExits(api, [STRANGER_ID]);
    expect(back.status, `could not set route.exits: ${JSON.stringify(back.body)}`).toBe(200);
    await expect.poll(() => routeState(api), { timeout: STATE_TIMEOUT_MS }).toBe('unavailable');

    const routeText = app.homePanel.routeState;
    const expectedText = `${STRANGER_LABEL} is offline — sites may not open or may open directly`;

    await test.step('DOM: #route-state is visible when unavailable', async () => {
      // The page polls the state itself (~2 s) — wait for the text against the fresh route.state.
      await expect(routeText).toBeVisible({ timeout: DOM_TIMEOUT_MS });
    });

    await test.step('#route-state: the verbatim text "<id> is offline — sites may not open or may open directly"', async () => {
      await expect(routeText).toHaveText(expectedText, { timeout: DOM_TIMEOUT_MS });
    });

    await test.step('the domains table has no path column', async () => {
      await app.tabs.open('domains');
      const domains = new DomainsSection(page);
      await expect(domains.headers.first()).toBeVisible();
      await domains.expectHeadersMatchSnapshot();
    });
  },
);
