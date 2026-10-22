import { test, expect } from '@playwright/test';
import { classifyConsole, classifyCspEvents } from './console';

test.describe('classifyConsole', () => {
  test('network autolog: "Failed to load resource" without a CSP marker — networkAutoLogs, not pageErrors', () => {
    const verdict = classifyConsole([
      { type: 'error', text: 'Failed to load resource: the server responded with a status of 404' },
    ]);

    expect(verdict.networkAutoLogs).toHaveLength(1);
    expect(verdict.pageErrors).toHaveLength(0);
  });

  test('script error: arbitrary error-level text without markers — pageErrors', () => {
    const verdict = classifyConsole([{ type: 'error', text: 'TypeError: undefined is not a function' }]);

    expect(verdict.pageErrors).toHaveLength(1);
  });

  test('CSP with a network phrase in one entry — both pageErrors and cspViolations, but not networkAutoLogs', () => {
    const verdict = classifyConsole([
      {
        type: 'error',
        text: 'Failed to load resource ... Content Security Policy directive...',
      },
    ]);

    expect(verdict.pageErrors).toHaveLength(1);
    expect(verdict.cspViolations).toHaveLength(1);
    expect(verdict.networkAutoLogs).toHaveLength(0);
  });

  test('page exception (type "pageerror") — pageErrors', () => {
    const verdict = classifyConsole([{ type: 'pageerror', text: 'ReferenceError: x is not defined' }]);

    expect(verdict.pageErrors).toHaveLength(1);
  });

  test('non-error level ("warning") with a network marker is not counted at all', () => {
    const verdict = classifyConsole([{ type: 'warning', text: 'Failed to load resource: 404' }]);

    expect(verdict.pageErrors).toHaveLength(0);
    expect(verdict.networkAutoLogs).toHaveLength(0);
    expect(verdict.cspViolations).toHaveLength(0);
  });

  test('securitypolicyviolation events become lines with the directive and the source', () => {
    const lines = classifyCspEvents([
      { directive: 'script-src', blockedUri: 'inline', sourceFile: 'http://h/', line: 3 },
      { directive: 'img-src', blockedUri: 'http://x/a.png' },
    ]);

    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain('script-src');
    expect(lines[0]).toContain('http://h/:3');
    expect(lines[1]).toContain('img-src');
    expect(lines[1]).not.toContain('(');
  });

  test('without securitypolicyviolation events the list is empty', () => {
    expect(classifyCspEvents([])).toEqual([]);
  });
});
