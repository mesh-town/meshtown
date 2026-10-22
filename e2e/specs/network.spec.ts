import { test, expect } from '../fixtures/index';
import type { MeshApi } from '../fixtures/api';
import { NET_NAME } from '../pages/glossary';
import { DiagnosticsSection } from '../pages/network-section';

interface InviteDoc {
  invite?: string;
}

const START_TIMEOUT_MS = 15000;
const INVITE_PREFIX = 'mesh-invite2-';

async function fetchInvite(api: MeshApi): Promise<{ status: number; invite: string | undefined }> {
  const res = await api.getJson<InviteDoc>('/api/network/invite');
  return { status: res.status, invite: res.status === 200 ? res.body?.invite : undefined };
}

// A real node answers 409 right after startup (the node is starting) — wait until the network is up.
async function awaitNetwork(api: MeshApi): Promise<void> {
  await expect.poll(async () => (await fetchInvite(api)).status, { timeout: START_TIMEOUT_MS }).toBe(200);
}

test('15-invite: invite: GET /api/network/invite and the button put it on the clipboard', async ({ app, api, page }) => {
  const net = new DiagnosticsSection(page);
  await awaitNetwork(api);
  const first = await fetchInvite(api);

  await test.step('a network exists: GET /api/network/invite -> 200, invite starts with mesh-invite2-', async () => {
    expect.soft(first.status).toBe(200);
    expect.soft(first.invite?.startsWith(INVITE_PREFIX)).toBe(true);
  });

  // The button lives only on "Home" (opened by default) and does not move to another tab.
  await net.trapClipboard();
  // The invite changes every second (`invite::make`) and the button makes its own GET, so the
  // responses of `/api/network/invite` are collected by a fetch interceptor in the page (before the click), not by polls from the test.
  await page.evaluate(() => {
    const w = window as unknown as { __inv: string[] };
    w.__inv = [];
    const origFetch = window.fetch;
    window.fetch = async (input, init) => {
      const res = await origFetch(input, init);
      if (String(input).includes('/api/network/invite')) {
        void res
          .clone()
          .json()
          .then((b: { invite?: string }) => {
            if (b.invite) w.__inv.push(b.invite);
          })
          .catch(() => undefined);
      }
      return res;
    };
  });
  await net.inviteButton.click();
  await expect.poll(() => net.clipboard(), { timeout: 6000 }).not.toBeNull();
  const clip = await net.clipboard();
  const seen = await page.evaluate(() => (window as unknown as { __inv: string[] }).__inv);

  await test.step('a network exists: clicking #netkey-invite-btn puts the invite on the clipboard', async () => {
    expect.soft(clip?.startsWith(INVITE_PREFIX)).toBe(true);
    expect.soft(clip !== null && seen.includes(clip), 'the clipboard matches the reply the page received').toBe(true);
  });

  await test.step('a network exists: the invite line under the counters shows the same text', async () => {
    await expect.soft(net.inviteValue).toBeVisible();
    await expect.soft(net.inviteValue).toHaveText(clip ?? '');
  });

  void app;
});

test.describe('no network', () => {
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('15-invite (no network): GET /api/network/invite -> 409 busy', async ({ api, targetKind }) => {
    test.skip(targetKind === 'demo', 'DemoNode does not distinguish the network state');

    await test.step('no network: GET /api/network/invite -> 409 busy', async () => {
      const res = await api.getJson<{ error?: string }>('/api/network/invite');
      expect.soft(res.status).toBe(409);
      expect.soft(res.body?.error).toBe('busy');
    });
  });
});

test('60-diagnostics-summary: "Copy summary for help" puts a summary without the key, the mask and id_hex on the clipboard; the key is visible only after "Show" and is stored nowhere', async ({ app, api, page }) => {
  await awaitNetwork(api);
  const net = new DiagnosticsSection(page);
  await app.tabs.open('network');
  await net.trapClipboard();
  const snapshot = await api.getJson<{ network?: { masked?: string; id_hex?: string } }>('/api/state');
  const masked = snapshot.body?.network?.masked;
  const idHex = snapshot.body?.network?.id_hex;
  expect(masked, '/api/state without network.masked: the network did not come up').toBeTruthy();
  expect(idHex, '/api/state without network.id_hex').toBeTruthy();

  await expect(net.checks).toContainText(new RegExp(`${NET_NAME} (is running|has been running for)`));
  await net.summaryButton.click();

  await test.step('status "Summary copied"', async () => {
    await expect.soft(net.summaryStatus).toHaveText('Summary copied');
  });

  await test.step('clipboard: starts with Mesh Town, says that Mesh Town is running, no invite, mask or id_hex', async () => {
    const clip = (await net.clipboard()) ?? '';
    expect.soft(clip.startsWith('Mesh Town')).toBe(true);
    expect.soft(clip).toMatch(new RegExp(`${NET_NAME} (is running|has been running for)`));
    expect.soft(clip).not.toContain(INVITE_PREFIX);
    expect.soft(clip).not.toContain('mesh-invite1-');
    // The whole clipboard is checked, including the log lines: the node prints the page token, the network and node ids there.
    const nodeId = await api.getJson<{ node?: { id?: string; id_full?: string } }>('/api/state');
    const ids = [idHex, nodeId.body?.node?.id, nodeId.body?.node?.id_full];
    expect.soft(clip, 'key mask in the summary').not.toContain(masked ?? '\u0000');
    expect.soft(clip, 'token= with a value in the summary').not.toMatch(/token=(?!‹hidden›)/);
    for (const id of ids) {
      expect.soft(clip, 'network or node id in the summary').not.toContain(id || '\u0000');
    }
    expect.soft(clip, 'long hex (the tail of the full node id) in the summary').not.toMatch(/[0-9a-f]{40,}/i);
  });

  await net.openSpecialist();
  await net.keyShowButton.click();
  await expect.soft(page.locator('#netkey')).not.toHaveText(masked ?? '');
  const shown = await page.locator('#netkey').innerText();

  await test.step('"Show": the key is visible in #netkey and is not in storages or title attributes', async () => {
    expect.soft(shown, 'the key is shown in place of the mask').not.toBe(masked);
    expect.soft(shown.length).toBeGreaterThan(0);
    const leaks = await page.evaluate((secret) => {
      const stores = [window.localStorage, window.sessionStorage];
      const inStorage = stores.some((st) => Object.keys(st).some((k) => (st.getItem(k) ?? '').includes(secret)));
      const inTitle = Array.from(document.querySelectorAll('[title]')).some((el) => (el.getAttribute('title') ?? '').includes(secret));
      return { inStorage, inTitle };
    }, shown);
    expect.soft(leaks.inStorage, 'key in localStorage/sessionStorage').toBe(false);
    expect.soft(leaks.inTitle, 'key in a title attribute').toBe(false);
  });

  await test.step('"Hide": the mask again', async () => {
    await net.keyHideButton.click();
    await expect.soft(page.locator('#netkey')).toHaveText(masked ?? '');
  });
});
