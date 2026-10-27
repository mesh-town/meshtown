import type { APIRequestContext, Page } from '@playwright/test';

import { test, expect } from '../fixtures/index';
import type { MeshApi } from '../fixtures/api';
import type { RunningTarget } from '../fixtures/target';
import { InviteForm, NetTabs, OutOfNetCard } from '../pages/invite-form';
import type { AppShell } from '../pages/app-shell';

// One form takes an invite wherever the computer meets one: the node, not the user, picks the slot. Each case checks the tab
// the user lands on and the slots the node ends up with, so a form that always joins the main network fails here.

const READY_TIMEOUT_MS = 15000;
/** The demo network every keyless demo node hands an invite for; no demo slot is in it at the start. */
const NEW_NETWORK = 'demo-invited';

interface InviteDoc {
  invite?: string;
}

interface NetsDoc {
  nets?: { id: string; id_hex: string | null }[];
}

interface SlotDoc {
  slot?: string;
}

interface Reply<T> {
  status: number;
  body: T | null;
}

/** The invite a slot hands out; waits until a real node has its network up. */
async function inviteOf(api: MeshApi, slot: string | null): Promise<string> {
  const path = slot === null ? '/api/network/invite' : `/api/net/${slot}/network/invite`;
  let invite = '';
  await expect
    .poll(async () => {
      const res = await api.getJson<InviteDoc>(path);
      invite = res.body?.invite ?? '';
      return res.status;
    }, { timeout: READY_TIMEOUT_MS })
    .toBe(200);
  return invite;
}

/** POST with the token, the host and the origin the page itself sends. */
async function post<T>(request: APIRequestContext, target: RunningTarget, path: string, data: unknown): Promise<Reply<T>> {
  const res = await request.fetch(`${target.origin}${path}?token=${target.token}`, {
    method: 'POST',
    headers: { Host: target.host, Origin: target.origin },
    data,
    failOnStatusCode: false,
    maxRedirects: 0,
  });
  const text = await res.text();
  return { status: res.status(), body: text === '' ? null : (JSON.parse(text) as T) };
}

/** An empty slot (no key yet) hands out an invite to the demo network no slot is in. */
async function inviteToNewNetwork(api: MeshApi, request: APIRequestContext, target: RunningTarget): Promise<{ invite: string; emptySlot: string }> {
  const added = await post<SlotDoc>(request, target, '/api/nets', {});
  expect(added.status, 'POST /api/nets adds an empty slot').toBe(201);
  const emptySlot = added.body?.slot ?? '';
  return { invite: await inviteOf(api, emptySlot), emptySlot };
}

/** The slots that hold a network; the empty slots waiting for a key are not counted. */
async function visibleNets(api: MeshApi): Promise<string[]> {
  const res = await api.getJson<NetsDoc>('/api/nets');
  return (res.body?.nets ?? []).filter((n) => n.id_hex !== null).map((n) => n.id);
}

/** The "+" tab of the Networks page: the card that joins or creates another network. */
async function openAddTab(app: AppShell, page: Page): Promise<void> {
  await app.tabs.open('peers');
  await new NetTabs(page).addTab.click();
  await app.tabs.expectShown('peers');
}

test.describe('invite into a new network (demo, two networks)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a new network needs the demo registry; a real node has one network');
  test.use({ targetFlags: { extraArgs: ['--nets', '2'] } });

  test('110-invite-new-slot: an invite to another network pasted in the "+" tab takes a new slot and opens its tab', async ({ app, page, api, request, target }) => {
    const tabs = new NetTabs(page);
    const form = new InviteForm(page, 'add');
    const { invite, emptySlot } = await inviteToNewNetwork(api, request, target);
    await page.goto(`${target.origin}/#peers/add`);
    await app.tabs.expectShown('peers');
    const before = await tabs.slots();

    await test.step('given: two network tabs; the empty slot made for the invite is not shown', async () => {
      expect(before).toHaveLength(2);
      expect(before).not.toContain(emptySlot);
    });

    await test.step('the pasted invite is read as a new network: no "already in" note', async () => {
      await form.pasteAndInspect(invite);
      await expect(form.placeNote).toHaveCount(0);
    });

    await test.step('"Add network" opens the tab of the slot the node took for it', async () => {
      await form.submit();
      await expect.poll(() => tabs.slots()).toHaveLength(3);
      const added = (await tabs.slots()).filter((s) => !before.includes(s));
      expect(added, 'one new tab, not the main one and not the empty slot').toHaveLength(1);
      await expect.poll(() => tabs.hash()).toBe(`#peers/${added[0]}`);
      await expect(tabs.tab(added[0])).toHaveAttribute('aria-current', 'page');
      await expect(tabs.tab(added[0])).toContainText(NEW_NETWORK);
    });

    await test.step('the first network is still the main one: the invite did not replace it', async () => {
      await expect(tabs.tab(before[0])).not.toHaveAttribute('aria-current', 'page');
      expect(await visibleNets(api)).toEqual(expect.arrayContaining(before));
    });
  });

  test('115-invite-api-addresses: the unnamed join address picks the slot, a slot address joins that slot', async ({ api, request, target }) => {
    const { invite, emptySlot } = await inviteToNewNetwork(api, request, target);
    const own = await inviteOf(api, null);
    const before = await visibleNets(api);

    await test.step('an invite of a network this node is in: 409 "already added", it names the slot, nothing is joined', async () => {
      const res = await post<{ error?: string; slot?: string }>(request, target, '/api/network/join', { key: own });
      expect(res.status).toBe(409);
      expect(res.body?.error).toBe('net_already_added');
      expect(res.body?.slot).toBe('main');
      expect(await visibleNets(api)).toEqual(before);
    });

    await test.step('the same invite to the slot address of the empty slot: that slot is entered, as it always was', async () => {
      const res = await post<{ slot?: string }>(request, target, `/api/net/${emptySlot}/network/join`, { key: invite });
      expect(res.status).toBe(202);
      expect(res.body?.slot).toBe(emptySlot);
      await expect.poll(() => visibleNets(api)).toContain(emptySlot);
    });

    await test.step('the same invite on the unnamed address: 409 "already added", it names the slot the invite went to', async () => {
      const res = await post<{ error?: string; slot?: string; label?: string }>(request, target, '/api/network/join', { key: invite });
      expect(res.status).toBe(409);
      expect(res.body?.error).toBe('net_already_added');
      expect(res.body?.slot).toBe(emptySlot);
      expect(res.body?.label).toBe(NEW_NETWORK);
    });
  });

  test('116-invite-api-new-network: the unnamed join address takes an invite to a new network into a new slot, not into main', async ({ api, request, target }) => {
    const { invite } = await inviteToNewNetwork(api, request, target);
    const before = await visibleNets(api);
    const res = await post<{ slot?: string }>(request, target, '/api/network/join', { key: invite });
    expect(res.status).toBe(202);
    expect(res.body?.slot, 'the answer names the slot the invite went to').toBeTruthy();
    expect(res.body?.slot).not.toBe('main');
    await expect.poll(() => visibleNets(api)).toEqual([...before, res.body?.slot]);
  });
});

test('111-invite-already-here: your own invite on the "+" tab says "already in", "Open" shows the members of that network, no slot is taken', async ({ app, page, api }) => {
  const form = new InviteForm(page, 'add');
  const own = await inviteOf(api, null);
  const slotsBefore = await visibleNets(api);
  await openAddTab(app, page);

  await test.step('the pasted invite is marked as one of this computer\'s networks', async () => {
    await form.pasteAndInspect(own);
    await expect(form.placeNote).toContainText("You're already in");
    expect(await form.placeNote.textContent(), 'a space between the sentence and "Open"').toMatch(/\. Open/);
  });

  await test.step('"Add network" does not join: the block says "already in" and offers "Open"', async () => {
    await form.submit();
    await expect(form.alreadyHere).toContainText("You're already in");
    await expect(form.openButton).toBeVisible();
    await expect(form.error).toBeHidden();
    await expect(page.getByText(/Open "/).filter({ visible: true }), 'one "Open" on the screen').toHaveCount(1);
  });

  await test.step('"Open" leads to the Networks tab of that network', async () => {
    await form.openButton.click();
    await app.tabs.expectShown('peers');
    await expect.poll(() => page.evaluate(() => location.hash)).toBe('#peers/main');
  });

  await test.step('the node still has the same networks: the invite was not spent on a new slot', async () => {
    expect(await visibleNets(api)).toEqual(slotsBefore);
  });
});

test.describe('this computer removed from the second network (demo --nets 2 --me-excluded)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a real node has one network and is never removed at the start');
  test.use({ targetFlags: { extraArgs: ['--nets', '2', '--me-excluded'] } });

  test('113-invite-excluded-tab: the card button of a network tab opens the form on that tab and puts the cursor into it', async ({ app, page }) => {
    const tabs = new NetTabs(page);
    const card = new OutOfNetCard(page, 'members');
    const form = new InviteForm(page, 'rejoin');
    await app.tabs.open('peers');
    const second = (await tabs.slots())[1];

    await test.step('given: the second tab shows the card of a removed computer', async () => {
      await tabs.tab(second).click();
      await expect.poll(() => tabs.hash()).toBe(`#peers/${second}`);
      await expect(card.card).toBeVisible();
    });

    await test.step('the button opens the form on this tab and puts the cursor into it', async () => {
      await card.joinButton.click();
      await expect(form.input).toBeFocused();
      expect(await tabs.hash(), 'still the second tab, not Home').toBe(`#peers/${second}`);
      await expect(form.refused, 'nothing was sent, so nothing is refused').toBeHidden();
      await expect(form.status).toBeHidden();
    });
  });

  test('118-invite-excluded-tab-joins: the invite of the removed network brings the computer back in this slot only', async ({ app, page, api }) => {
    const tabs = new NetTabs(page);
    const card = new OutOfNetCard(page, 'members');
    const form = new InviteForm(page, 'rejoin');
    await app.tabs.open('peers');
    const [main, second] = await tabs.slots();
    const invite = await inviteOf(api, second);
    await tabs.tab(second).click();
    await card.joinButton.click();
    await form.pasteAndInspect(invite);
    await form.submit();
    await expect(card.card).toHaveCount(0);
    await expect(page.getByText('Invite sent'), 'the old status line is gone').toHaveCount(0);
    await expect(page.getByText("Couldn't join")).toHaveCount(0);
    expect(await tabs.hash()).toBe(`#peers/${second}`);
    await tabs.tab(main).click();
    await expect(card.card).toBeVisible();
  });
});

test.describe('the setup wizard (demo --no-network)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a keyless demo node hands out an invite to a network it knows; a real node has none');
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('114-invite-wizard: an invite pasted in the wizard joins the first network and the wizard goes to its next step', async ({ app, api, page }) => {
    const form = new InviteForm(page, 'home');
    const invite = await inviteOf(api, null);

    await test.step('given: no network, the wizard offers "Join with an invite"', async () => {
      await expect(app.netCard.startTitle).toBeVisible();
      await app.netCard.startJoin.click();
      await expect(form.input).toBeVisible();
    });

    await test.step('the invite is read as a new network; "Join" sends it', async () => {
      await form.pasteAndInspect(invite);
      await form.submit();
    });

    await test.step('the wizard says the computer joined, and the node reports a network', async () => {
      await expect(app.wizard.joined).toContainText('✓ You joined the network');
      await expect(app.wizard.progress).toHaveText(/^Step 2 of \d$/);
      await expect.poll(async () => (await api.getJson<{ network?: unknown }>('/api/state')).body?.network).toBeTruthy();
    });

    await test.step('the first network lives in the main slot: no extra slot is taken', async () => {
      expect(await visibleNets(api)).toEqual(['main']);
    });
  });
});

test.describe('an invite pasted on the "+" tab into another network (demo, two networks)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a new network needs the demo registry; a real node has one network');
  test.use({ targetFlags: { extraArgs: ['--nets', '2'] } });

  test('119-invite-settings-arrival: the tab of the new network opens, and the "+" tab holds an empty form afterwards', async ({ app, page, api, request, target }) => {
    const tabs = new NetTabs(page);
    const form = new InviteForm(page, 'add');
    const { invite } = await inviteToNewNetwork(api, request, target);
    const before = await visibleNets(api);
    await openAddTab(app, page);
    await form.pasteAndInspect(invite);
    await form.submit();

    await test.step('the address becomes the tab of the slot the node took, and the row has one tab more', async () => {
      await expect.poll(async () => (await tabs.hash()).startsWith('#peers/')).toBe(true);
      await app.tabs.expectShown('peers');
      await expect(tabs.slotTabs).toHaveCount(before.length + 1);
      const added = (await tabs.slots()).filter((s) => !before.includes(s));
      expect(added).toHaveLength(1);
      expect(await tabs.hash()).toBe(`#peers/${added[0]}`);
      await expect(tabs.tab(added[0])).toHaveAttribute('aria-current', 'page');
    });

    await test.step('back on the "+" tab the form is empty and does not wait for the network any more', async () => {
      await openAddTab(app, page);
      await expect(form.input).toHaveValue('');
      await expect(page.getByText('Joining the network…')).toHaveCount(0);
    });
  });

  test('120-invite-address: an invite in the page address is in the field with its preview, and Join opens the new network', async ({ page, api, request, target }) => {
    const tabs = new NetTabs(page);
    const form = new InviteForm(page, 'add');
    const { invite } = await inviteToNewNetwork(api, request, target);
    const before = await visibleNets(api);
    await page.goto(`${target.origin}/?token=${target.token}&invite=${encodeURIComponent(invite)}`);

    await test.step('the node is in a network: the page goes to the "+" tab with the field filled and read', async () => {
      await expect.poll(() => tabs.hash()).toBe('#peers/add');
      await expect(form.input).toHaveValue(invite);
      await expect(form.kindPreview).toBeVisible();
    });

    await test.step('"Add network" opens the tab of the new network', async () => {
      await form.submit();
      await expect.poll(async () => (await tabs.slots()).filter((s) => !before.includes(s))).toHaveLength(1);
      const added = (await tabs.slots()).filter((s) => !before.includes(s));
      expect(await tabs.hash()).toBe(`#peers/${added[0]}`);
    });
  });

  test.describe('the limit of networks (demo --nets 7 and one more network)', () => {
    test.use({ targetFlags: { extraArgs: ['--nets', '7'] } });

    test('121-invite-limit-once: a new network at the limit is told about once above the field and once, shortly, under it', async ({ app, page, api, request, target }) => {
      const form = new InviteForm(page, 'add');
      const { invite, emptySlot } = await inviteToNewNetwork(api, request, target);
      // the eighth network is another one, so that the invite kept above is still new at the limit
      const eighth = await post<SlotDoc>(request, target, `/api/net/${emptySlot}/network/create`, { label: 'filler' });
      expect(eighth.status, 'the empty slot becomes a network of its own').toBeLessThan(300);
      await expect.poll(async () => (await visibleNets(api)).length).toBe(8);
      await openAddTab(app, page);
      await form.paste(invite);
      await form.submit();

      await expect(form.limitLine).toHaveCount(1);
      await expect(form.error).toHaveCount(1);
      await expect(form.error).toContainText("can't be added until you leave one");
      await expect(page.getByText(/is a new network, and Mesh Town is already in/)).toHaveCount(0);
      await expect(page.getByText('The new network gets its own tab')).toHaveCount(0);
    });
  });
});

test.describe('an invite in the address, no network yet (demo --no-network)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a keyless demo node hands out an invite to a network it knows; a real node has none');
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('120-invite-address-wizard: the wizard field is filled from the address, with its preview and note, and Join leads into the network', async ({ app, page, api, target }) => {
    const form = new InviteForm(page, 'home');
    const invite = await inviteOf(api, null);
    await page.goto(`${target.origin}/?token=${target.token}&invite=${encodeURIComponent(invite)}`);

    await expect(form.input).toHaveValue(invite);
    await expect(form.kindPreview).toBeVisible();
    await form.submit();
    await expect(app.wizard.joined).toContainText('✓ You joined the network');
  });
});

// The "Scan QR" link opens the scanner of the phone app; it exists only on a page opened in the app shell.
const SCAN_LINK = 'a.invite-scan';
const SCAN_URL = 'meshtown-app://scan-invite';

test.describe('the invite form in the phone app', () => {
  test.use({ userAgent: 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/120.0 Mobile Safari/537.36 MeshTownApp/Android' });

  test('122-invite-scan-app: the form has a "Scan QR" link to the app scanner right after its button', async ({ app, page }) => {
    const form = new InviteForm(page, 'add');
    await openAddTab(app, page);
    await expect(form.button).toBeVisible();

    // no anchor: the link is a styled anchor with a custom-scheme address, told apart by its class
    const scan = page.locator(SCAN_LINK);
    await expect(scan).toHaveCount(1);
    await expect(scan).toHaveText('Scan QR');
    await expect(scan).toHaveAttribute('href', SCAN_URL);
    await expect(form.button.locator('xpath=following-sibling::*[1]'), 'the link stands next to the form button').toHaveClass(/invite-scan/);
  });
});

test.describe('the invite form in a browser', () => {
  test('122-invite-scan-browser: the form has its button and no "Scan QR" link', async ({ app, page }) => {
    const form = new InviteForm(page, 'add');
    await openAddTab(app, page);
    await expect(form.button).toBeVisible();
    await expect(page.locator(SCAN_LINK)).toHaveCount(0);
  });
});
