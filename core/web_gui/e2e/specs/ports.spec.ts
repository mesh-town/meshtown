import { test, expect } from '../fixtures/index';
import { PortsSection } from '../pages/ports-section';

interface PortsCfg {
  config?: { ports?: { published?: { port?: number }[]; opened?: { local?: number }[] } };
}

interface UdpEntry {
  port?: number;
  proto?: string;
}
interface UdpCfg {
  config?: { ports?: { published?: UdpEntry[]; opened?: UdpEntry[] } };
}

test('48-ports-tab: tab "Ports": "Open" and "Close" a port of another member', async ({
  app,
  api,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the "Ports shared with you" row is a web_demo stub fixture; a real node has none');
  const ports = new PortsSection(page);
  const cfg = async () => (await api.getJson<PortsCfg>('/api/config')).body?.config?.ports;
  await app.tabs.open('ports');
  const row = ports.netRows.first();

  await test.step('"Ports shared with you" has a stub row', async () => {
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

  await test.step('the opened address has "Copy" and "Change number"; "Ports shared with you" comes before "My published ports"', async () => {
    await expect.soft(ports.copyLocalButton(row, local)).toHaveText('Copy');
    await expect.soft(ports.changeButton(row)).toBeVisible();
    const titles = await ports.cardTitles().allTextContents();
    expect.soft(titles.indexOf('Ports shared with you'), 'order of the cards').toBeLessThan(titles.indexOf('My published ports'));
    expect.soft(titles.indexOf('Ports shared with you'), 'the card exists').toBeGreaterThanOrEqual(0);
  });

  await test.step('"Close": the row has no "open" mark, ports.opened is empty', async () => {
    await ports.closeButton(row).click();
    await expect.soft(row, 'the row has no "open" mark').not.toContainText('open');
    await expect.poll(async () => (await cfg())?.opened, { message: 'ports.opened is empty', timeout: 6000 }).toEqual([]);
  });
});

test('ports-web-address: tab "Ports": a member\'s port has a link in the browser column and a "Copy" button (demo)', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the shared port row is a web_demo stub fixture; a real node has none');
  const ports = new PortsSection(page);
  await app.tabs.open('ports');
  await page.evaluate(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => undefined }, configurable: true });
  });
  const row = ports.netRows.first();
  const link = row.getByRole('link');
  const url = (await link.getAttribute('href')) ?? '';

  await test.step('the table has the columns Member, Port, In the browser, In other programs', async () => {
    const table = page.getByRole('table', { name: 'Ports shared with you' });
    await expect.soft(table.getByRole('columnheader')).toHaveText(['Member', 'Port', 'In the browser', 'In other programs']);
  });

  await test.step('the link is the member\'s .meshtown address, opens in a new tab and shows its own address', async () => {
    expect.soft(url, 'href').toMatch(/^http:\/\/[a-z0-9-]+\.meshtown:9000$/);
    await expect.soft(link).toHaveText(url);
    await expect.soft(link).toHaveAttribute('target', '_blank');
    await expect.soft(link).toHaveAttribute('rel', 'noopener');
  });

  await test.step('"Copy address <url>" says "Copied" after a click', async () => {
    const copy = row.getByRole('button', { name: `Copy address ${url}` });
    await expect.soft(copy).toHaveText('Copy');
    await copy.click();
    await expect.soft(copy).toHaveText('Copied');
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

test('123-ports-udp: tab "Ports": publishing UDP 8080 next to TCP 8080, the UDP badge, "Open" and "Close" of the shared UDP row, "Unpublish" (demo)', async ({
  app,
  api,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the shared UDP row is a web_demo stub fixture; a real node has none');
  const ports = new PortsSection(page);
  const cfg = async () => (await api.getJson<UdpCfg>('/api/config')).body?.config?.ports;
  await app.tabs.open('ports');

  await test.step('"Publish" TCP 8080 and UDP 8080: two rows, the badge only on the UDP one, PUT has proto "udp"', async () => {
    await ports.publish('8080', 'udp-web');
    await expect(ports.ownRow('8080', 'tcp'), 'TCP row').toHaveCount(1, { timeout: 6000 });
    await expect(ports.addPort, 'the form is cleared after publishing').toHaveValue('');
    const put = page.waitForRequest((r) => new URL(r.url()).pathname === '/api/config' && r.method() === 'PUT');
    await ports.publish('8080', 'udp-echo', 'udp');
    const body = (await put).postDataJSON() as { config?: { ports?: { published?: UdpEntry[] } } };
    const entry = body.config?.ports?.published?.find((p) => p.proto === 'udp');
    expect.soft(entry?.port, JSON.stringify(entry)).toBe(8080);
    await expect.soft(ports.ownRow('8080', 'tcp'), 'TCP row').toHaveCount(1, { timeout: 6000 });
    await expect.soft(ports.ownRow('8080', 'udp'), 'UDP row').toHaveCount(1, { timeout: 6000 });
    await expect.soft(ports.udpBadge, 'only the UDP rows have the badge').toHaveCount(2);
  });

  await test.step('the shared UDP row (demo-dns 5353) has the badge; "Open" puts proto "udp", "Close" removes it', async () => {
    const row = ports.netRows.filter({ has: ports.udpBadge });
    await expect.soft(row, 'one shared UDP row').toHaveCount(1);
    await expect.soft(row.getByRole('link'), 'no browser link for UDP').toHaveCount(0);
    await ports.openButton(row).click();
    await expect
      .poll(async () => (await cfg())?.opened?.map((o) => [o.proto, o.port]), { message: 'ports.opened', timeout: 6000 })
      .toEqual([['udp', 5353]]);
    await ports.closeButton(row).click();
    await expect.poll(async () => (await cfg())?.opened, { message: 'ports.opened is empty', timeout: 6000 }).toEqual([]);
  });

  await test.step('"Unpublish" of the UDP row leaves the TCP one', async () => {
    await ports.ownRow('8080', 'udp').getByRole('button', { name: 'Unpublish' }).click();
    await expect.soft(ports.ownRow('8080', 'udp'), 'UDP row is still there').toHaveCount(0, { timeout: 6000 });
    await expect.soft(ports.ownRow('8080', 'tcp'), 'TCP row is gone too').toHaveCount(1);
    await ports.ownRow('8080', 'tcp').getByRole('button', { name: 'Unpublish' }).click();
    await expect.poll(async () => (await cfg())?.published, { message: 'ports.published', timeout: 6000 }).toEqual([]);
  });
});
