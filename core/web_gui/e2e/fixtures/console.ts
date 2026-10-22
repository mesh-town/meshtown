// Browser console parsing — message classification.
import { expect, test as base } from '@playwright/test';
import type { ConsoleMessage, Page } from '@playwright/test';

export const NETWORK_AUTO_MARKER = 'Failed to load resource';
export const CSP_VIOLATION_MARKER = 'Content Security Policy';

export interface ConsoleEntry {
  type: string;
  text: string;
}

export interface ConsoleVerdict {
  pageErrors: string[];
  networkAutoLogs: string[];
  cspViolations: string[];
}

export function classifyConsole(entries: readonly ConsoleEntry[]): ConsoleVerdict {
  const verdict: ConsoleVerdict = { pageErrors: [], networkAutoLogs: [], cspViolations: [] };
  for (const entry of entries) classifyEntry(entry, verdict);
  return verdict;
}

function classifyEntry(entry: ConsoleEntry, verdict: ConsoleVerdict): void {
  if (entry.type === 'pageerror') {
    verdict.pageErrors.push(entry.text);
    return;
  }
  if (entry.type !== 'error') return;
  const isNetworkAuto = entry.text.includes(NETWORK_AUTO_MARKER) && !entry.text.includes(CSP_VIOLATION_MARKER);
  if (isNetworkAuto) {
    verdict.networkAutoLogs.push(entry.text);
    return;
  }
  verdict.pageErrors.push(entry.text);
  if (entry.text.includes(CSP_VIOLATION_MARKER)) verdict.cspViolations.push(entry.text);
}

// A CSP violation caught by the `securitypolicyviolation` event on the page.
export interface CspEvent {
  directive: string;
  blockedUri: string;
  sourceFile?: string;
  line?: number;
}

// Name of the array on `window` where the init script collects violations.
export const CSP_EVENTS_KEY = '__cspViolations';

// Pure classification: page events -> lines for `cspViolations`.
export function classifyCspEvents(events: readonly CspEvent[]): string[] {
  return events.map((e) => {
    const where = e.sourceFile ? ` (${e.sourceFile}:${e.line ?? 0})` : '';
    return `${CSP_VIOLATION_MARKER}: ${e.directive} blocked "${e.blockedUri}"${where}`;
  });
}

// Init script: attaches a listener before the page code runs and collects violations in an array on window.
const CSP_INIT_SCRIPT = `(() => {
  const list = (window.${CSP_EVENTS_KEY} = []);
  document.addEventListener('securitypolicyviolation', (e) => {
    list.push({
      directive: e.violatedDirective,
      blockedUri: e.blockedURI,
      sourceFile: e.sourceFile,
      line: e.lineNumber,
    });
  });
})();`;

export class ConsoleLog {
  private readonly entries: ConsoleEntry[] = [];
  private cspEvents: CspEvent[] = [];

  constructor(private readonly page: Page) {
    page.on('console', (msg: ConsoleMessage) => this.entries.push({ type: msg.type(), text: msg.text() }));
    page.on('pageerror', (err: Error) => this.entries.push({ type: 'pageerror', text: String(err) }));
  }

  // Installs a `securitypolicyviolation` listener in all future documents of the page.
  async installCspListener(): Promise<void> {
    await this.page.addInitScript(CSP_INIT_SCRIPT);
  }

  // Reads the collected violations; if the page is already closed or gone, keeps what was read earlier.
  async collectCspEvents(): Promise<void> {
    try {
      this.cspEvents = await this.page.evaluate((key) => (window as unknown as Record<string, CspEvent[]>)[key] ?? [], CSP_EVENTS_KEY);
    } catch {
      // the page is closed — nothing to read
    }
  }

  texts(): string[] {
    return this.entries.map((e) => e.text);
  }

  verdict(): ConsoleVerdict {
    const verdict = classifyConsole(this.entries);
    verdict.cspViolations.push(...classifyCspEvents(this.cspEvents));
    return verdict;
  }
}

export const test = base.extend<{ consoleLog: ConsoleLog }>({
  consoleLog: async ({ page }, use, testInfo) => {
    const log = new ConsoleLog(page);
    await log.installCspListener();
    await use(log);
    await log.collectCspEvents();
    const verdict = log.verdict();
    for (const text of verdict.networkAutoLogs) {
      testInfo.annotations.push({ type: 'console-network', description: text });
    }
    expect(verdict.pageErrors, 'page console errors').toEqual([]);
    expect(verdict.cspViolations, 'CSP violations (securitypolicyviolation)').toEqual([]);
  },
});
