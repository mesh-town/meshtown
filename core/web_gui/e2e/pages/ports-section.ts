import { expect, type Locator, type Page } from '@playwright/test';
import { patchMainState } from '../helpers/overview-route';

interface PortEntry {
  port?: number;
  local?: number;
  name?: string;
  for?: string;
}

/** The "Ports" tab (port publishing) and the "Keep publishing this computer's ports…" question when the network changes. */
export class PortsSection {
  readonly addPort: Locator;
  readonly addName: Locator;
  readonly addButton: Locator;
  readonly audienceAll: Locator;
  readonly audienceNodes: Locator;
  readonly ownBody: Locator;
  readonly netRows: Locator;
  readonly keepDialog: Locator;
  readonly keepCancel: Locator;
  readonly keepYes: Locator;
  readonly keepNo: Locator;
  readonly createLabel: Locator;
  readonly createToggle: Locator;
  readonly createButton: Locator;

  private recordedPosts: unknown[] = [];
  private published: PortEntry[] = [];
  private stateRouted = false;

  constructor(private readonly page: Page) {
    // Field names are their placeholders ("port", "name (optional)"); exact — the "Ports shared by members" rows have a "port number on this computer" field.
    this.addPort = page.getByRole('spinbutton', { name: 'port', exact: true });
    this.addName = page.getByRole('textbox', { name: 'name (optional)' });
    this.addButton = page.getByRole('button', { name: 'Publish', exact: true });
    this.audienceAll = page.getByRole('radio', { name: 'All members' });
    this.audienceNodes = page.getByRole('radio', { name: 'Selected members' });
    // Tables are told apart by their accessible name (`aria-label` on `<table>`).
    this.ownBody = page.getByRole('table', { name: 'My published ports' });
    this.netRows = page
      .getByRole('table', { name: 'Ports shared by members' })
      .getByRole('row')
      .filter({ has: page.getByRole('button', { name: /^(Open|Close)$/ }) });
    this.keepDialog = page.getByRole('dialog', { name: /Keep publishing this computer's ports/ });
    this.keepCancel = this.keepDialog.getByRole('button', { name: 'Cancel', exact: true });
    this.keepYes = this.keepDialog.getByRole('button', { name: 'Yes', exact: true });
    this.keepNo = this.keepDialog.getByRole('button', { name: 'No', exact: true });
    this.createLabel = page.getByLabel(/Network name \(members will see it/);
    this.createToggle = page.getByRole('link', { name: /No invite\? Create a new/ });
    this.createButton = page.getByRole('button', { name: 'Create network', exact: true });
  }

  async publish(port: string, name: string): Promise<void> {
    await this.addPort.fill(port);
    await this.addName.fill(name);
    await this.addButton.click();
  }

  /** The node's checkbox in the "Selected members" list by node name (the label starts with the name). */
  audienceNode(name: string): Locator {
    return this.page.getByRole('checkbox', { name: new RegExp(`^${name}`) });
  }

  localInput(row: Locator): Locator {
    return row.getByRole('spinbutton', { name: 'port number on this computer' });
  }

  openButton(row: Locator): Locator {
    return row.getByRole('button', { name: 'Open', exact: true });
  }

  closeButton(row: Locator): Locator {
    return row.getByRole('button', { name: 'Close', exact: true });
  }

  /** The network name field: expands the collapsed "Create" block if it is hidden. */
  async revealCreateLabel(): Promise<Locator> {
    if (!(await this.createLabel.isVisible())) {
      await this.createToggle.click();
    }
    return this.createLabel;
  }

  async dialogOpen(): Promise<boolean> {
    // A closed `<dialog>` is not visible and is not in the accessibility tree: open ⇔ visible.
    return this.keepDialog.isVisible();
  }

  /** "No" is the main button of the question: its background differs from that of "Yes" and "Cancel" (computed style is a measurement, not a selector). */
  async keepNoIsPrimary(): Promise<boolean> {
    const bg = (b: Locator) => b.evaluate((el) => getComputedStyle(el).backgroundColor);
    const [no, yes, cancel] = await Promise.all([bg(this.keepNo), bg(this.keepYes), bg(this.keepCancel)]);
    return no !== yes && no !== cancel;
  }

  async focusedId(): Promise<string | null> {
    return this.page.evaluate(() => (document.activeElement ? document.activeElement.id : null));
  }

  /**
   * Publications are mixed into the main network state of the `GET /api/overview` response (`page.route`): the page draws what
   * the poll brought. The method waits for the `/api/overview` response with the substitution (the body has the needed number of publications), then for the table rows.
   */
  async setPublished(list: PortEntry[]): Promise<void> {
    this.published = list;
    if (!this.stateRouted) {
      this.stateRouted = true;
      await patchMainState(this.page, (state: { ports?: object }) => ({
        ...state,
        ports: Object.assign({}, state.ports, { published: this.published }),
      }));
    }
    // The Ports tab may be hidden (the page is on Home), so we wait for the poll response with
    // the needed number of publications, not for table rows: they are not always visible.
    await this.page.waitForResponse(async (r) => {
      if (new URL(r.url()).pathname !== '/api/overview' || r.request().method() !== 'GET') return false;
      const body = await r.json().catch(() => null);
      const main = ((body && body.slots) || []).find((s: { slot?: string }) => s.slot === 'main');
      return ((main && main.state && main.state.ports && main.state.ports.published) || []).length === list.length;
    });
    // The response arrived, but the snapshot reaches the store later (`acceptSnapshot`, and on the first snapshot also
    // `refreshConfig`): wait for the "My published ports" rows. A hidden tab does not show them, but a CSS locator counts them.
    // no anchor: a table row without a role, distinguishable from the "empty" row only by the `data-port-unpublish` button
    const rows = this.page.locator('#sec-ports [data-port-unpublish]');
    await expect.poll(() => rows.count()).toBe(list.length);
  }

  /** Network interception: the join and create-network POSTs do not reach the node, bodies accumulate in `recordedPosts`. */
  async recordNetworkPosts(): Promise<void> {
    this.recordedPosts = [];
    await this.page.route(/\/api\/network\/(join|create)$/, async (route) => {
      if (route.request().method() !== 'POST') {
        await route.continue();
        return;
      }
      this.recordedPosts.push(route.request().postDataJSON());
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"label":"test","restart":true}' });
    });
  }

  async waitPosts(n: number): Promise<void> {
    await expect.poll(() => this.recordedPosts.length).toBeGreaterThanOrEqual(n);
  }

  async posts(): Promise<unknown[]> {
    return [...this.recordedPosts];
  }
}
