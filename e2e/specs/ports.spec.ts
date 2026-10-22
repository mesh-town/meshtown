import { test, expect } from '../fixtures/index';
import { PortsSection } from '../pages/ports-section';

interface PortsCfg {
  config?: { ports?: { published?: { port?: number }[]; opened?: { local?: number }[] } };
}

test('48-ports-tab: tab "Ports": "Open" and "Close" a port of another member', async ({
  app,
  api,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the "Ports shared by members" row is a web_demo stub fixture; a real node has none');
  const ports = new PortsSection(page);
  const cfg = async () => (await api.getJson<PortsCfg>('/api/config')).body?.config?.ports;
  await app.tabs.open('ports');
  const row = ports.netRows.first();

  await test.step('"Ports shared by members" has a stub row', async () => {
    await expect.soft(row, 'no row').toBeVisible();
  });


  const local = await ports.localInput(row).inputValue();

  await test.step('"Open": 127.0.0.1:<number> is visible and ports.opened has one entry with this local', async () => {
    await ports.openButton(row).click();
    await expect.soft(row, '127.0.0.1:<the publisher\'s number> is visible').toContainText(`127.0.0.1:${local}`);
    await expect
      .poll(async () => (await cfg())?.opened?.map((o) => o.local), { message: 'ports.opened', timeout: 6000 })
      .toEqual([Number(local)]);
  });

  await test.step('"Close": the row has no "open" mark, ports.opened is empty', async () => {
    await ports.closeButton(row).click();
    await expect.soft(row, 'the row has no "open" mark').not.toContainText('open');
    await expect.poll(async () => (await cfg())?.opened, { message: 'ports.opened is empty', timeout: 6000 }).toEqual([]);
  });
});

test('57-ports-publish-unpublish: tab "Ports": publishing 8080 via the form, a row in "My published ports" and "Unpublish" (demo and real node)', async ({
  app,
  api,
  page,
}) => {
  const ports = new PortsSection(page);
  const cfg = async () => (await api.getJson<PortsCfg>('/api/config')).body?.config?.ports;
  await app.tabs.open('ports');

  await test.step('"Publish" 8080: a row in "My published ports" and an entry in ports.published', async () => {
    await ports.publish('8080', 'webcheck-web');
    await expect.soft(ports.ownBody, 'a row in "My published ports"').toContainText('8080', { timeout: 6000 });
    await expect
      .poll(async () => (await cfg())?.published?.some((p) => p.port === 8080), { message: 'ports.published', timeout: 6000 })
      .toBe(true);
  });

  await test.step('"Unpublish": no row, ports.published without 8080', async () => {
    await ports.ownBody.getByRole('button', { name: 'Unpublish' }).click();
    await expect.soft(ports.ownBody, 'the row is still there').not.toContainText('8080', { timeout: 6000 });
    await expect
      .poll(async () => (await cfg())?.published?.some((p) => p.port === 8080), { message: 'ports.published', timeout: 6000 })
      .toBe(false);
  });
});

test('74-ports-publish-nodes: tab "Ports": publishing 8081 only to the selected node peer-one, "Shared with" in the table and "Unpublish" (demo)', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'nodes peer-one and peer-two are a web_demo stub fixture; a real node has other members');
  const ports = new PortsSection(page);
  await app.tabs.open('ports');

  await test.step('"Selected members", tick peer-one, "Publish" 8081: PUT /api/config has an entry with its full id', async () => {
    await ports.audienceNodes.check();
    await ports.audienceNode('peer-one').check();
    await ports.addPort.fill('8081');
    const put = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/config' && r.method() === 'PUT');
    await ports.addButton.click();
    const body = (await put).postDataJSON() as { config?: { ports?: { published?: { port?: number; for?: unknown }[] } } };
    const entry = body.config?.ports?.published?.find((p) => p.port === 8081);
    expect.soft(Array.isArray(entry?.for) && (entry?.for as string[]).length === 1, JSON.stringify(entry)).toBe(true);
  });

  await test.step('in "My published ports" "Shared with" = peer-one', async () => {
    const row = ports.ownBody.getByRole('row').filter({ hasText: '8081' });
    await expect.soft(row, 'column "Shared with"').toContainText('peer-one', { timeout: 6000 });
  });

  await test.step('"Unpublish": no 8081 row', async () => {
    const row = ports.ownBody.getByRole('row').filter({ hasText: '8081' });
    await row.getByRole('button', { name: 'Unpublish' }).click();
    await expect.soft(ports.ownBody, 'the row is still there').not.toContainText('8081', { timeout: 6000 });
  });
});

test('49-ports-keep-dialog: a question about port publications before joining and creating a network: "No" by default, Escape without a request, keep_ports in the body', async ({
  app,
  page,
}) => {
  const ports = new PortsSection(page);
  const join = app.joinCard;
  const pub = [{ port: 8080, name: '', for: 'all' }];
  const invite = 'webcheck-invite';
  await join.reveal(app);
  await ports.recordNetworkPosts();
  const enter = async () => {
    await join.keyInput.fill(invite);
    await join.joinButton.click();
  };

  await ports.setPublished(pub);
  await enter();

  await test.step('"Join" with publications: the dialog is open, no request yet', async () => {
    await expect.poll(() => ports.dialogOpen(), { message: 'the dialog is open' }).toBe(true);
    expect.soft(await ports.posts(), 'no requests').toEqual([]);
  });

  await test.step('focus is on #ports-keep-no, it has the primary button class', async () => {
    await expect.poll(() => ports.focusedId(), { message: 'focus' }).toBe('ports-keep-no');
    expect.soft(await ports.keepNoIsPrimary(), 'btn-primary').toBe(true);
  });

  await ports.keepYes.click();
  await ports.waitPosts(1);

  await test.step('"Yes" — a body with key and keep_ports: true', async () => {
    expect.soft(await ports.posts()).toEqual([{ key: invite, keep_ports: true }]);
  });

  await ports.setPublished(pub);
  await enter();
  await page.keyboard.press('Escape');

  await test.step('Escape — the dialog is closed, no new bodies (the previous "Yes" did not resurface)', async () => {
    await expect.poll(() => ports.dialogOpen(), { message: 'the dialog is closed' }).toBe(false);
    expect.soft((await ports.posts()), 'exactly one body').toHaveLength(1);
  });

  await ports.setPublished(pub);
  await enter();
  await page.keyboard.press('Enter');
  await ports.waitPosts(2);

  await test.step('Enter — a body with keep_ports: false', async () => {
    expect.soft((await ports.posts())[1]).toEqual({ key: invite, keep_ports: false });
  });

  await ports.setPublished([]);
  await enter();
  await ports.waitPosts(3);

  await test.step('no publications — no dialog, a body without keep_ports', async () => {
    expect.soft(await ports.dialogOpen(), 'no dialog').toBe(false);
    const last = (await ports.posts()).at(-1) as Record<string, unknown>;
    expect.soft('keep_ports' in last, JSON.stringify(last)).toBe(false);
  });

  const label = await ports.revealCreateLabel();
  await ports.setPublished(pub);
  await label.fill('webcheck-net');
  await ports.createButton.click();
  await ports.keepNo.click();
  await ports.waitPosts(4);

  await test.step('"Create network", "No" — a body {label, keep_ports: false}', async () => {
    const p = await ports.posts();
    expect.soft(p).toHaveLength(4);
    expect.soft(p[3]).toEqual({ label: 'webcheck-net', keep_ports: false });
  });
});
