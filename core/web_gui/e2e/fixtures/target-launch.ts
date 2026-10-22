// Target process (demo/node): launch, readiness wait, token, stop.
// Only `launchTarget` is visible outside — the `target` fixture (`target.ts`) calls it alone.
import { spawn, type ChildProcess } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { constants as fsConstants } from 'node:fs';
import { access, mkdir, open, readFile } from 'node:fs/promises';
import * as net from 'node:net';
import * as path from 'node:path';

import { expect } from '@playwright/test';

import { forgetPid, e2eOutDir, recordPid } from './orphans';
import { buildNodeArgs, nodeKeygen, prepareNodeConfig, resolveNodePlan } from './node-config';
import type { RunningTarget, TargetFlags, TargetKind } from './target';

export interface LaunchSpec {
  kind: TargetKind;
  bin: string;
  flags: Required<TargetFlags>;
  workDir: string;
}

export interface TargetHandle {
  running: RunningTarget;
  stop(): Promise<void>;
}

export async function launchTarget(spec: LaunchSpec): Promise<TargetHandle> {
  await assertBinExists(spec.bin);
  const port = await freePort();
  const socksPort = spec.flags.socksOverride ? await freePortExcept(port) : undefined;
  await mkdir(spec.workDir, { recursive: true });
  const logPath = path.join(spec.workDir, 'target.log');
  const host = `127.0.0.1:${port}`;
  const origin = `http://${host}`;

  const plan = await planLaunch(spec, port, socksPort);
  const child = await spawnChild(spec.bin, plan.args, logPath, await privateTmpDir(spec.workDir));
  try {
    await raceReady(child, origin, plan.readyTimeoutMs, logPath);
    if (plan.readyLogMarker) await waitLogLine(logPath, plan.readyLogMarker, plan.readyTimeoutMs);
    const token = await plan.resolveToken(spec.workDir);
    await checkOwnTarget(origin, token);
    return {
      running: { kind: spec.kind, port, socksPort, host, origin, token, logPath },
      stop: () => stopProcess(child),
    };
  } catch (err) {
    await stopProcess(child).catch(() => undefined);
    throw err;
  }
}

async function assertBinExists(bin: string): Promise<void> {
  const ok = await access(bin, fsConstants.X_OK).then(
    () => true,
    () => false,
  );
  if (!ok) {
    throw new Error(`no binary ${bin}: make web-check-nobuild / E2E_DEMO_BIN (E2E_NODE_BIN)`);
  }
}

// A port for the override flag — free and different from the port of the target's web page.
async function freePortExcept(taken: number): Promise<number> {
  const port = await freePort();
  return port === taken ? freePortExcept(taken) : port;
}

function withSocksOverride(flags: Required<TargetFlags>, socksPort?: number): Required<TargetFlags> {
  if (socksPort === undefined) return flags;
  return { ...flags, extraArgs: [...flags.extraArgs, '--cli-override', `socks.port=${socksPort}`] };
}

export async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.on('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const address = srv.address();
      if (address === null || typeof address === 'string') {
        srv.close(() => reject(new Error('freePort: the server gave no numeric address')));
        return;
      }
      const { port } = address;
      srv.close(() => resolve(port));
    });
  });
}

interface LaunchPlan {
  readyTimeoutMs: number;
  // The log line after which the node is ready (the web comes up earlier than the hyper transport: ~5 s).
  readyLogMarker?: string;
  args: string[];
  resolveToken(workDir: string): Promise<string>;
}

// Target readiness over HTTP: 20 s — under host load (load 40–130) 6 s was not enough; one constant for both targets.
const READY_TIMEOUT_MS = 20000;

// The only place that parses the target kind (Five Lines of Code — one enum parse for the whole file):
// the rest of the code (launchTarget, spawnChild, raceReady…) gets a ready LaunchPlan, and `spec.kind` is
// no longer checked. Node specifics (config and keygen) are in `node-config.ts`; the plan from the flags
// is assembled here, in its branch.
async function planLaunch(spec: LaunchSpec, port: number, socksPort?: number): Promise<LaunchPlan> {
  const flags = withSocksOverride(spec.flags, socksPort);
  switch (spec.kind) {
    case 'demo': {
      const token = demoToken();
      return {
        readyTimeoutMs: READY_TIMEOUT_MS,
        args: demoArgs(port, token, flags),
        resolveToken: async () => token,
      };
    }
    case 'node': {
      const plan = resolveNodePlan(spec.flags.extraArgs, socksPort);
      const configPath = await prepareNodeConfig(spec.workDir, plan.fileKind);
      const key = await nodeKeygen(spec.bin);
      return {
        readyTimeoutMs: READY_TIMEOUT_MS,
        readyLogMarker: plan.fileKind === 'asis' ? '[node] mode:' : undefined,
        args: buildNodeArgs(key, configPath, port, plan),
        resolveToken: (workDir) => readNodeToken(workDir),
      };
    }
  }
}

// The demo puts its config in `$TMPDIR/mesh-web-demo-<pid>`; the shared OS directory survives SIGKILL and
// on a repeated pid gives a foreign directory (permissions 000/555 from earlier cases) — the target then exits
// before becoming ready. A separate TMPDIR per target (inside the tree's workDir) rules out the collision.
async function privateTmpDir(workDir: string): Promise<string> {
  const dir = path.join(workDir, 'tmp');
  await mkdir(dir, { recursive: true });
  return dir;
}

async function spawnChild(
  bin: string,
  args: string[],
  logPath: string,
  tmpDir: string,
): Promise<ChildProcess> {
  const log = await open(logPath, 'a');
  const child = spawn(bin, args, {
    stdio: ['ignore', log.fd, log.fd],
    env: { ...process.env, TMPDIR: tmpDir },
  });
  child.once('exit', () => void log.close());
  if (child.pid !== undefined) recordPid(pidsDir(), child.pid, process.pid, [bin, ...args].join(' '));
  return child;
}

export function demoArgs(port: number, token: string, flags: Required<TargetFlags>): string[] {
  return [
    '--port',
    String(port),
    '--token',
    token,
    '--seed',
    String(flags.seed),
    '--fast',
    String(flags.fast),
    ...flags.extraArgs,
  ];
}

export function demoToken(): string {
  return `e2e-${treeTag()}-${randomUUID()}`;
}

export function treeTag(): string {
  const root = path.resolve(__dirname, '..', '..', '..', '..', '..');
  return createHash('sha256').update(root).digest('hex').slice(0, 8);
}

export async function checkOwnTarget(origin: string, token: string): Promise<void> {
  const res = await fetch(`${origin}/api/config?token=${token}`).catch(() => null);
  if (res === null || res.status !== 200) {
    throw new Error(`port ${new URL(origin).port}: a foreign target answers (port race)`);
  }
}

async function waitReady(origin: string, timeoutMs: number): Promise<void> {
  const pollStatus = async (): Promise<number> => {
    const res = await fetch(`${origin}/`).catch(() => null);
    return res === null ? 0 : res.status;
  };
  await expect.poll(pollStatus, { timeout: timeoutMs, intervals: [50] }).toBe(200);
}

async function raceReady(
  child: ChildProcess,
  origin: string,
  timeoutMs: number,
  logPath: string,
): Promise<void> {
  const diedEarly = once(child, 'exit').then(() => {
    throw new Error(`target exited before becoming ready, log: ${logPath}`);
  });
  diedEarly.catch(() => undefined);
  await Promise.race([waitReady(origin, timeoutMs), diedEarly]);
}

async function waitLogLine(logPath: string, marker: string, timeoutMs: number): Promise<void> {
  const has = async (): Promise<boolean> =>
    (await readFile(logPath, 'utf8').catch(() => '')).includes(marker);
  await expect.poll(has, { timeout: timeoutMs, intervals: [100], message: `the log has no "${marker}"` }).toBe(true);
}

export async function readNodeToken(dir: string): Promise<string> {
  const tokenPath = path.join(dir, 'web-token');
  let token = '';
  const readOnce = async (): Promise<string> => {
    token = await readFile(tokenPath, 'utf8')
      .then((text) => text.trim())
      .catch(() => '');
    return token;
  };
  await expect
    .poll(readOnce, {
      timeout: 2000,
      intervals: [50],
      message: `node token did not appear within 2 s: ${tokenPath}`,
    })
    .not.toBe('');
  return token;
}

export async function stopProcess(child: ChildProcess): Promise<void> {
  await terminate(child);
  if (child.pid !== undefined) forgetPid(pidsDir(), child.pid);
}

function pidsDir(): string {
  return path.join(e2eOutDir(), 'pids');
}

async function terminate(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill('SIGTERM');
  if (await waitExit(child, 5000)) return;
  child.kill('SIGKILL');
  await waitExit(child, 5000);
}

async function waitExit(child: ChildProcess, timeoutMs: number): Promise<boolean> {
  try {
    await once(child, 'exit', { signal: AbortSignal.timeout(timeoutMs) });
    return true;
  } catch {
    return false;
  }
}
