import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { patchMainState } from '../helpers/overview-route';
import { NETS_TEXT, NetsBlock, type Rgba } from '../pages/home/nets-block';

// Choosing networks for the proxy and the page. Neither the real node nor the demo node reports networks,
// so the main state of `/api/overview` (and `/api/config` for the laptop) is replaced: the real demo reply plus the networks fields.
// Saving ("Save") is not checked in the demo: it does not know the `ifaces` field yet — saving is covered by vitest.

const NARROW = { width: 390, height: 844 };
const ROUTER_PAC = 'http://192.168.88.1:1080/proxy.pac';

test('77-nets-router-wizard: a router with two networks — the Overview card and the setup instruction show the PAC address of the home network, the summary "The proxy is open for: Home network. The ISP network is closed automatically.", "Edit" expands the list, ticking the ISP network gives a yellow warning, the "you are here now" network cannot be unticked, no horizontal scroll at 390', async ({
  page,
  target,
  app,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the state mock turns the demo into a router; a real node does not pretend to be a router');
  const nets = new NetsBlock(page);
  const wizard = app.wizard;
  const { homeNet, providerNet } = NETS_TEXT;

  await test.step('given: the page sees a router with networks br-lan (open) and eth0 (ISP network, closed automatically); the Overview tab is open', async () => {
    await pretendState(page, routerState);
    await app.open(target.origin, target.token);
    await wizard.openOverview();
  });

  await test.step('the Overview card shows the PAC address of the home network (not 127.0.0.1) and the paragraph "…of a phone or computer connected to the router"', async () => {
    await expect.soft(app.proxyCard.addressText).toHaveText(ROUTER_PAC);
    await expect.soft(app.proxyCard.hint).toHaveText(NETS_TEXT.pacIntroRouter);
  });

  await test.step('the setup instruction shows the same address; the intro paragraph stays on Overview', async () => {
    await wizard.open();
    await wizard.toGuide('system', 'linux');
    await expect.soft(app.proxyCard.addressText).toHaveText(ROUTER_PAC);
    await expect.soft(app.proxyCard.hint).toHaveCount(0);
  });

  await test.step('the summary "The proxy is open for: Home network. The ISP network is closed automatically." with an "Edit" button; the list is collapsed', async () => {
    await expect.soft(nets.summary).toBeVisible();
    await expect.soft(nets.summary).toContainText(NETS_TEXT.summaryAuto);
    await expect.soft(nets.editButton).toHaveAttribute('aria-expanded', 'false');
    await expect.soft(nets.block).toBeHidden();
  });

  await test.step('"Edit" expands the list in place: the home network is ticked, the ISP network is not and is labeled "closed automatically", a note says the choice is automatic', async () => {
    await nets.editButton.click();
    await expect.soft(nets.editButton).toHaveAttribute('aria-expanded', 'true');
    await expect.soft(nets.block).toBeVisible();
    await expect.soft(nets.title).toBeVisible();
    await expect.soft(nets.checkbox(homeNet)).toBeChecked();
    await expect.soft(nets.checkbox(providerNet)).toBeChecked({ checked: false });
    await expect.soft(nets.badge(providerNet, NETS_TEXT.badgeAutoClosed)).toBeVisible();
    await expect.soft(nets.autoNote).toHaveText(NETS_TEXT.autoNote);
    await expect.soft(nets.warning('eth0'), 'warning before ticking').toBeHidden();
    await expect.soft(nets.saveButton, 'no changes yet').toBeDisabled();
  });

  await test.step('ticking the ISP network: a yellow warning under the row, "Save" is enabled (we do not click the button)', async () => {
    await nets.checkbox(providerNet).check();
    await expect.soft(nets.warning('eth0')).toBeVisible();
    await expect.soft(nets.warning('eth0')).toHaveText(NETS_TEXT.warnProvider);
    // Assumption: "yellow" means a box with a warm yellow background (like `--warn-bg` of `.warn-strip`); the spec does not fix the exact color
    const tint = await nets.background(nets.warning('eth0'));
    expect.soft(isWarmYellow(tint), `the warning background is not yellow: ${JSON.stringify(tint)}`).toBe(true);
    await expect.soft(nets.saveButton).toBeEnabled();
  });

  await test.step('the "you are here now" network (br-lan): badge, checkbox disabled; clicking the label or the checkbox does not untick it and shows a note', async () => {
    const here = nets.checkbox(homeNet);
    await expect.soft(nets.badge(homeNet, NETS_TEXT.badgeHere)).toBeVisible();
    await expect.soft(here).toBeDisabled();
    await expect.soft(nets.hereNote, 'note before the click').toBeHidden();
    // dispatchEvent: Playwright does not click a disabled element, and the lock is made for exactly that
    await nets.row(homeNet).getByText(homeNet, { exact: true }).dispatchEvent('click');
    await expect.soft(nets.hereNote).toHaveText(NETS_TEXT.hereNote);
    await expect.soft(here, 'after clicking the label').toBeChecked();
    await here.dispatchEvent('click');
    await expect.soft(here, 'after clicking the checkbox').toBeChecked();
    await expect.soft(nets.hereNote).toBeVisible();
  });

  await test.step('at 390 there is no horizontal scroll: the summary, the list and the warning stay within the window', async () => {
    await app.resize(NARROW);
    await expect.poll(() => app.scrollWidth(), 'width of the scrollable page').toBeLessThanOrEqual(NARROW.width);
    expect.soft(await app.right(nets.summary), 'right edge of the summary').toBeLessThanOrEqual(NARROW.width);
    expect.soft(await app.right(nets.block), 'right edge of the network list').toBeLessThanOrEqual(NARROW.width);
    expect.soft(await app.right(nets.warning('eth0')), 'right edge of the warning').toBeLessThanOrEqual(NARROW.width);
  });
});

test('78-nets-laptop-quiet: a laptop with one network — the setup instruction has no networks summary, the "Which local networks can connect" block is neither on Home nor in "Settings"', async ({
  page,
  target,
  app,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the state mock turns the demo into a laptop with one network; a real node does not mock networks');
  const nets = new NetsBlock(page);

  await test.step('given: the page sees a laptop with the only network en0; the proxy and the page are open "to the local network", so one network gives the quiet, not the mode', async () => {
    await pretendState(page, laptopState);
    await pretendConfig(page, openToLocalnet);
    await app.open(target.origin, target.token);
  });

  await test.step('"Home → Overview": the browser card is in place with the old paragraph and the address (127.0.0.1), no networks block', async () => {
    await expect.soft(app.proxyCard.card).toBeVisible();
    await expect.soft(app.proxyCard.hint).toHaveText(NETS_TEXT.pacIntroLaptop);
    await expect.soft(app.proxyCard.addressText).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/proxy\.pac$/);
    await expect.soft(nets.block).toHaveCount(0);
    await expect.soft(nets.title).toHaveCount(0);
  });

  await test.step('the setup instruction: the address is the old one (127.0.0.1), no summary and no "Edit"', async () => {
    await app.wizard.open();
    await app.wizard.toGuide('system', 'linux');
    await expect.soft(app.proxyCard.addressText).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/proxy\.pac$/);
    await expect.soft(nets.summary).toHaveCount(0);
    await expect.soft(nets.editButton).toHaveCount(0);
    await expect.soft(nets.block).toHaveCount(0);
    await expect.soft(nets.title).toHaveCount(0);
  });

  await test.step('"Settings": the modes are "local network" (the mock got through), and there is no networks block and no "From the same networks" line — neither in General nor in Advanced', async () => {
    await app.tabs.open('settings');
    await expect.soft(nets.socksAccess).toHaveValue('localnet');
    await app.settings.openExpert();
    await expect.soft(nets.webAccess).toHaveValue('localnet');
    await expect.soft(nets.block).toHaveCount(0);
    await expect.soft(nets.title).toHaveCount(0);
    await expect.soft(nets.sameNets).toHaveCount(0);
  });
});

interface StateDoc {
  node?: object;
  socks?: object;
  [key: string]: unknown;
}

interface ConfigDoc {
  config?: { socks?: object; web?: object };
  [key: string]: unknown;
}

/** Returns one entry of `node.networks`, shaped the way the node reports it. */
function net(name: string, addr: string, defaultRoute: boolean, open: boolean): object {
  return { name, addrs: [addr], default_route: defaultRoute, present: true, open, by: 'auto' };
}

/** Returns the router state: br-lan is open by auto choice and is "you are here now", eth0 is the ISP network, closed automatically. */
function routerState(body: StateDoc): StateDoc {
  return {
    ...body,
    node: {
      ...body.node,
      router: true,
      via_iface: 'br-lan',
      networks_side: 'socks',
      networks: [net('br-lan', '192.168.88.1', false, true), net('eth0', '192.168.1.100', true, false)],
    },
    socks: {
      ...body.socks,
      access: 'localnet',
      ifaces: null,
      lan: [{ addr: '192.168.88.1:1080', iface: 'br-lan' }],
      pac_url: ROUTER_PAC,
    },
    web: { access: 'localnet', listening: 'localnet', ifaces: null },
    restart_pending: [],
  };
}

/** Returns the laptop state: the only network en0 with a default route; the page is opened from the machine itself. */
function laptopState(body: StateDoc): StateDoc {
  return {
    ...body,
    node: {
      ...body.node,
      router: false,
      via_iface: null,
      networks_side: 'socks',
      networks: [net('en0', '192.168.1.23', true, true)],
    },
    socks: { ...body.socks, access: 'localnet', ifaces: null, lan: [{ addr: '192.168.1.23:1080', iface: 'en0' }] },
    web: { access: 'localnet', listening: 'localnet', ifaces: null },
    restart_pending: [],
  };
}

/** Returns the saved settings: both the proxy and the page are open "to the local network". */
function openToLocalnet(body: ConfigDoc): ConfigDoc {
  const config = body.config ?? {};
  return {
    ...body,
    config: { ...config, socks: { ...config.socks, access: 'localnet' }, web: { ...config.web, access: 'localnet' } },
  };
}

/** Routes the main network state of `/api/overview` so that the page gets the real demo reply rewritten by `patch`. */
async function pretendState(page: Page, patch: (body: StateDoc) => StateDoc): Promise<void> {
  await patchMainState<StateDoc>(page, patch);
}

/** Does the same for reading `/api/config`; writes (PUT) go to the demo as they are. */
async function pretendConfig(page: Page, patch: (body: ConfigDoc) => ConfigDoc): Promise<void> {
  await page.route('**/api/config', async (route) => {
    if (route.request().method() !== 'GET') {
      await route.fallback();
      return;
    }
    const res = await route.fetch();
    const body = (await res.json()) as ConfigDoc;
    await route.fulfill({ response: res, json: patch(body) });
  });
}

/** Returns whether the color is the warm yellow background of a warning box — on the light theme (#fbe9ab) and the dark one (#4a3c07): red ≥ green ≫ blue. */
function isWarmYellow({ r, g, b, a }: Rgba): boolean {
  return a > 0.5 && r >= g && r > b + 40 && g > b + 30 && r - g < 60;
}
