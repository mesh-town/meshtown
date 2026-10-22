import path from 'node:path';
import { defineConfig } from '@playwright/test';
import { e2eOutDir } from './fixtures/orphans';
import type { TargetWorkerOptions } from './fixtures/target';

const ROOT = path.resolve(__dirname, '..', '..', '..', '..');
const DEFAULT_DEMO_BIN = path.join(ROOT, '.tmp', 'build', 'host', 'debug', 'examples', 'web_demo');
const DEFAULT_NODE_BIN = path.join(ROOT, '.tmp', 'build', 'host', 'debug', 'meshtown');

const OUT = e2eOutDir();

const SPEC_MATCH = ['specs/**/*.spec.ts', 'fixtures/**/*.test.ts'];
// The fixture tests do not read targetKind/targetBin and never contact the target — a second run in the node project would give the same results.
const FIXTURE_TESTS = 'fixtures/**/*.test.ts';

export default defineConfig<TargetWorkerOptions>({
  globalSetup: './fixtures/global-setup.ts',
  testDir: '.',
  outputDir: path.join(OUT, 'results'),
  reporter: [
    ['list'],
    ['html', { outputFolder: path.join(OUT, 'report'), open: 'never' }],
  ],
  preserveOutput: 'failures-only',
  timeout: 60000,
  retries: 0,
  fullyParallel: true,
  workers: 4,
  use: {
    headless: process.env.WEB_E2E_HEADED !== '1',
    viewport: { width: 1280, height: 800 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    // an action hanging for over 10 s is almost always a broken selector, not a slow target
    actionTimeout: 10_000,
  },
  projects: [
    {
      name: 'demo',
      testMatch: SPEC_MATCH,
      use: {
        targetKind: 'demo',
        targetBin: process.env.E2E_DEMO_BIN ?? DEFAULT_DEMO_BIN,
      },
    },
    {
      name: 'node',
      testMatch: SPEC_MATCH,
      testIgnore: FIXTURE_TESTS,
      use: {
        targetKind: 'node',
        targetBin: process.env.E2E_NODE_BIN ?? DEFAULT_NODE_BIN,
      },
    },
  ],
});
