import type { Locator, Page } from '@playwright/test';

import { NET_NAME } from './glossary';

/** The top part of the page: the tab strip (`role=tablist`), the network logo in it, and the old header. */
export class TopPanel {
  readonly tabList: Locator;
  /** The network name inside the tab strip — by text. */
  readonly logo: Locator;
  /** The old separate header `<header class="topbar">` — role `banner`. */
  readonly oldHeader: Locator;
  /** The node status line: the `banner` header contains only it (renamed from `HomeFieldsExtra.statusLine`). */
  readonly statusLine: Locator;
  /** "You haven't joined a Mesh Town network yet" — exactly, inside the status line. */
  readonly noNetworkStatus: Locator;

  constructor(page: Page) {
    this.tabList = page.locator('nav#tabs');
    this.logo = this.tabList.getByText(NET_NAME, { exact: true });
    this.oldHeader = page.getByRole('banner');
    // the status line is a live region `role="status"` in the column header (`banner`)
    this.statusLine = page.getByRole('banner').getByRole('status');
    // the phrase also occurs in sections ("Members") — look only inside the status line
    this.noNetworkStatus = this.statusLine.filter({ hasText: new RegExp(`^\\s*You haven't joined a ${NET_NAME} network yet\\s*$`) });
  }

  /** Returns the top of the locator's box; no box — `null` (the element is not rendered). */
  async top(locator: Locator): Promise<number | null> {
    const box = await locator.boundingBox();
    return box ? box.y : null;
  }

  /** Returns whether the old header is above the tab strip; if there is no header at all — `above: false`. */
  async oldHeaderAbove(): Promise<{ above: boolean; detail: string }> {
    if ((await this.oldHeader.count()) === 0) {
      return { above: false, detail: 'element .topbar not found — there is no separate header at all' };
    }
    const tabsTop = await this.top(this.tabList.first());
    const headerTop = await this.top(this.oldHeader.first());
    const above = tabsTop !== null && headerTop !== null && headerTop < tabsTop;
    return { above, detail: `.topbar above the panel: topbarTop=${headerTop}, tabsTop=${tabsTop}` };
  }
}
