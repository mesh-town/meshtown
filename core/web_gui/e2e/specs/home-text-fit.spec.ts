import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { patchMainState } from '../helpers/overview-route';

// The member ring and the exit list on "Home": the text does not hit the arc, the exit caption does not overlap the name.
// The roster is replaced via `patchMainState` (the main network state of the overview): any number of members without changing the node.

const GAP_PX = 4;
const WIDE = { width: 1280, height: 900 };
const NARROW = { width: 375, height: 900 };

type Peer = Record<string, unknown> & { self?: boolean };
interface Box { x: number; y: number; width: number; height: number }
interface RingGeometry { center: { x: number; y: number }; inner: number; texts: { name: string; box: Box }[] }

async function mockPeers(page: Page, total: number, online: number): Promise<void> {
  await patchMainState(page, (body: { peers?: Peer[] }) => {
    const self = (body.peers ?? []).find((p: { self?: boolean }) => p.self);
    const template = (body.peers ?? []).find((p: { self?: boolean }) => !p.self) ?? self;
    const others = Array.from({ length: total }, (_, i) => ({
      ...template,
      id: `fit${i}`,
      id_full: `fit${i}`.padEnd(64, '0'),
      name: `peer-${i}`,
      self: false,
      exit: i === 0,
      online: i < online,
      path: i < online ? (i === 0 ? 'relay' : 'direct') : null,
      rtt_ms: i < online ? 30 : null,
    }));
    return { ...body, peers: [self as Peer, ...others] };
  });
}

// A rectangle around the text itself (Range), not the stretched span; the inner radius comes from the svg and arc attributes.
async function ringGeometry(page: Page): Promise<RingGeometry> {
  return page.evaluate(() => {
    const ring = document.querySelector('.ring-mark') as HTMLElement;
    const svg = ring.querySelector('svg') as SVGElement;
    const arc = ring.querySelector('path') ?? ring.querySelector('circle');
    const size = Number(svg.getAttribute('width'));
    const thick = Number(arc?.getAttribute('stroke-width') ?? 0);
    const r = svg.getBoundingClientRect();
    const textBox = (el: Element): Box | null => {
      if (!el.textContent) return null;
      const range = document.createRange();
      range.selectNodeContents(el);
      const b = range.getBoundingClientRect();
      return { x: b.x, y: b.y, width: b.width, height: b.height };
    };
    const texts: { name: string; box: Box }[] = [];
    for (const cls of ['ring-center-main', 'ring-center-sub']) {
      const el = ring.querySelector('.' + cls);
      const box = el && getComputedStyle(el).display !== 'none' ? textBox(el) : null;
      if (box) texts.push({ name: cls, box });
    }
    return { center: { x: r.x + size / 2, y: r.y + size / 2 }, inner: size / 2 - thick, texts };
  });
}

function worstCorner(g: RingGeometry): number {
  const dists = g.texts.flatMap(({ box }) =>
    [[box.x, box.y], [box.x + box.width, box.y], [box.x, box.y + box.height], [box.x + box.width, box.y + box.height]]
      .map(([x, y]) => Math.hypot(x - g.center.x, y - g.center.y)),
  );
  return Math.max(...dists);
}

for (const [total, online] of [[40, 35], [999, 990]] as const) {
  test(
    `74-ring-text-fit-${total}: the "${online} of ${total}" count and "online" lie inside the inner circle of the ring with a gap ≥ ${GAP_PX} px (1280 and 375)`,
    async ({ page, target, app, targetKind }) => {
      test.skip(targetKind === 'node', 'the member roster mock is designed for the demo target');
      await mockPeers(page, total, online);
      await app.open(target.origin, target.token);
      for (const [label, size] of [['1280', WIDE], ['375', NARROW]] as const) {
        await test.step(`width ${label}`, async () => {
          await app.resize(size);
          await expect.poll(async () => (await ringGeometry(page)).texts[0]?.box.width ?? 0).toBeGreaterThan(0);
          await expect(page.locator('.ring-center-main')).toContainText(`${online} of ${total}`);
          const g = await ringGeometry(page);
          const worst = worstCorner(g);
          expect.soft(worst, `the farthest corner of the text is ${worst.toFixed(1)} px from the center, limit ${g.inner - GAP_PX}`).toBeLessThanOrEqual(g.inner - GAP_PX);
        });
      }
    },
  );
}

const overlap = (a: Box, b: Box): boolean =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

test(
  '75-exit-meta-no-overlap-375: at 375 the exit caption does not cross the member name, at 1280 it sits to the right of it',
  async ({ page, target, app, targetKind }) => {
    test.skip(targetKind === 'node', 'the member roster mock is designed for the demo target');
    await mockPeers(page, 6, 6);
    await app.open(target.origin, target.token);
    const rects = async (): Promise<{ label: Box; meta: Box }> =>
      page.evaluate(() => {
        const row = document.querySelector('#exit-list .exit-row:has(.exit-choice)') as HTMLElement;
        const pick = (sel: string): Box => {
          const range = document.createRange();
          range.selectNodeContents(row.querySelector(sel) as Element);
          const b = range.getBoundingClientRect();
          return { x: b.x, y: b.y, width: b.width, height: b.height };
        };
        return { label: pick('.exit-label-text'), meta: pick('.exit-meta') };
      });

    await test.step('375: the name and caption rectangles do not overlap, the caption is below the name', async () => {
      await app.resize(NARROW);
      await expect(page.locator('#exit-list .exit-meta').first()).toContainText('via relay server');
      const { label, meta } = await rects();
      expect.soft(overlap(label, meta), `the name ${JSON.stringify(label)} and the caption ${JSON.stringify(meta)} overlap`).toBe(false);
      expect.soft(meta.y, 'the caption is below the name').toBeGreaterThanOrEqual(label.y + label.height - 1);
    });

    await test.step('1280: the caption is to the right of the name, on the same line', async () => {
      await app.resize(WIDE);
      const { label, meta } = await rects();
      expect.soft(overlap(label, meta)).toBe(false);
      expect.soft(meta.x, 'the caption is to the right of the name').toBeGreaterThanOrEqual(label.x + label.width);
    });
  },
);
