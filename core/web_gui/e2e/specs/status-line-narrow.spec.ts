import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';

// The header with two networks and the proxy in the second one: "Sites open via <member> (<network>)" is long.
// At 390 px it wraps instead of being cut with "…", and the page does not grow sideways.
// The demo's proxy always sits in the first network, so the overview reply is rewritten (as in nets.spec.ts for the state).

const NARROW = { width: 390, height: 844 };
const EXIT_ID = 'ab'.repeat(32);
const LONG_MEMBER = 'office-gateway-frankfurt';
const LONG_NET = 'family and friends network';
const VIA = `Sites open via ${LONG_MEMBER} (${LONG_NET})`;
const TEXT_TIMEOUT_MS = 10000;

interface Peer {
  self?: boolean;
  [key: string]: unknown;
}
interface Brief {
  id: string;
  label?: string | null;
  proxy?: boolean;
  exits?: string[];
  route_state?: string;
}
interface Slot {
  slot: string;
  state?: { peers?: Peer[]; route?: unknown } | null;
}
interface Overview {
  nets?: Brief[];
  slots?: Slot[];
}

// The second network carries the proxy and offers the exit the proxy uses.
async function pretendProxyInSecondNet(page: Page): Promise<void> {
  await page.route('**/api/overview', async (route) => {
    const res = await route.fetch();
    const body = (await res.json()) as Overview;
    const [lead, other] = body.nets ?? [];
    const slot = (body.slots ?? []).find((s) => s.slot === other?.id);
    if (lead && other && slot?.state) {
      const exit = { id: EXIT_ID.slice(0, 8), id_full: EXIT_ID, name: LONG_MEMBER, country: null, exit: true, online: true, active: true };
      lead.proxy = false;
      Object.assign(other, { proxy: true, label: LONG_NET, exits: [EXIT_ID], route_state: 'exit' });
      slot.state.peers = [...(slot.state.peers ?? []).filter((p) => p.self), { ...exit, version: null, path: 'direct', rtt_ms: 20 }];
      slot.state.route = { state: 'exit', exits: [exit] };
    }
    await route.fulfill({ response: res, json: body });
  });
}

test.describe('two networks (demo --nets 2), header at 390', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a real node has one network');
  test.use({ targetFlags: { extraArgs: ['--nets', '2'] }, viewport: NARROW });

  test('status-line-narrow-390: a long "Sites open via … (…)" wraps inside the window: the text is not cut and the page has no horizontal scroll', async ({
    page,
    app,
    target,
  }) => {
    const line = app.topPanel.statusLine;
    const text = line.locator('.status-text');

    await test.step('given: two networks, the proxy and its exit are in the second one', async () => {
      await pretendProxyInSecondNet(page);
      await app.open(target.origin, target.token);
      await expect(line).toContainText(VIA, { timeout: TEXT_TIMEOUT_MS });
    });

    await test.step('the whole #status-line is inside the 390 px window', async () => {
      const box = await line.boundingBox();
      expect.soft(box?.x ?? -1, 'left edge of #status-line').toBeGreaterThanOrEqual(0);
      expect.soft(await app.right(line), 'right edge of #status-line').toBeLessThanOrEqual(NARROW.width);
    });

    await test.step('the text is not cut: .status-text scrollWidth <= clientWidth', async () => {
      const m = await text.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
      expect.soft(m.scrollWidth, `the text is cut with "…" (clientWidth ${m.clientWidth})`).toBeLessThanOrEqual(m.clientWidth);
    });

    await test.step('the page has no horizontal scroll', async () => {
      expect.soft(await app.scrollWidth(), 'the page is wider than the window').toBeLessThanOrEqual(NARROW.width);
    });
  });
});
