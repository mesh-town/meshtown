// Types and fixture of the target (demo/node) — starting and stopping the target.
import { access } from 'node:fs/promises';
import * as path from 'node:path';

import { test as base, type TestInfo } from '@playwright/test';

import { launchTarget, type LaunchSpec } from './target-launch';

export type TargetKind = 'demo' | 'node';

export interface TargetFlags {
  seed?: number;
  fast?: number;
  extraArgs?: readonly string[];
  // The flag `--cli-override socks.port=<N>` with N free at launch time: a hard-coded port would be shared by
  // parallel workers (the node holds it with a socket). N is in `RunningTarget.socksPort`.
  socksOverride?: boolean;
}

export interface RunningTarget {
  kind: TargetKind;
  port: number;
  socksPort?: number;
  host: string;
  origin: string;
  token: string;
  logPath: string;
}

export interface TargetWorkerOptions {
  targetKind: TargetKind;
  targetBin: string;
}

export interface TargetTestOptions {
  targetFlags: TargetFlags;
}

function resolveFlags(flags: TargetFlags): Required<TargetFlags> {
  return {
    seed: flags.seed ?? 1,
    fast: flags.fast ?? 20,
    extraArgs: flags.extraArgs ?? [],
    socksOverride: flags.socksOverride ?? false,
  };
}

// Attaches the target log to the report as `<kind>-log` (lands in `test-results/…` of a failed test).
async function attachLog(testInfo: TestInfo, kind: TargetKind, logPath: string): Promise<void> {
  const exists = await access(logPath).then(() => true, () => false);
  if (exists) await testInfo.attach(`${kind}-log`, { path: logPath, contentType: 'text/plain' });
}

export const test = base.extend<TargetTestOptions & { target: RunningTarget }, TargetWorkerOptions>({
  targetKind: ['demo', { scope: 'worker', option: true }],
  targetBin: ['', { scope: 'worker', option: true }],
  targetFlags: [{}, { option: true }],
  target: async ({ targetKind, targetBin, targetFlags }, use, testInfo) => {
    const spec: LaunchSpec = {
      kind: targetKind,
      bin: targetBin,
      flags: resolveFlags(targetFlags),
      workDir: testInfo.outputPath('target'),
    };
    const logPath = path.join(spec.workDir, 'target.log');
    // A failed launch (readiness wait) leaves no handle, but the log is already on disk.
    const handle = await launchTarget(spec).catch(async (err: unknown) => {
      await attachLog(testInfo, targetKind, logPath);
      throw err;
    });
    await use(handle.running);
    await handle.stop();
    if (testInfo.status !== testInfo.expectedStatus) await attachLog(testInfo, targetKind, logPath);
  },
});
