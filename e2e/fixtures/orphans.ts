// Registry of target (demo/node) pids and cleanup of orphans left over from a killed run.
// Cleanup kills a process only if the worker that recorded it is dead and the pid's command line matches
// the recorded one in full; there is no search by process name here.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import * as path from 'node:path';

export interface Probe {
  alive(pid: number): boolean;
  command(pid: number): string | null;
  kill(pid: number): void;
}

// E2E_RUN_TAG separates the result directories of parallel runs in one tree.
export function e2eOutDir(): string {
  const root = path.resolve(__dirname, '..', '..', '..');
  const tag = process.env.E2E_RUN_TAG;
  return path.join(root, '.tmp', tag ? `e2e-${tag}` : 'e2e');
}

export function recordPid(dir: string, pid: number, owner: number, cmd: string): void {
  mkdirSync(dir, { recursive: true });
  // The write is atomic: a parallel cleanup will not see an empty file (the .tmp name does not pass the ^\d+$ filter).
  const tmp = path.join(dir, `.${pid}.tmp`);
  writeFileSync(tmp, `${owner}\n${cmd}\n`);
  renameSync(tmp, path.join(dir, String(pid)));
}

export function forgetPid(dir: string, pid: number): void {
  rmSync(path.join(dir, String(pid)), { force: true });
}

export const realProbe: Probe = {
  alive(pid) {
    try {
      process.kill(pid, 0);
      return true;
    } catch (err) {
      return (err as NodeJS.ErrnoException).code === 'EPERM';
    }
  },
  command(pid) {
    try {
      const out = execFileSync('ps', ['-ww', '-p', String(pid), '-o', 'command='], { encoding: 'utf8' });
      return out.trim() === '' ? null : out.trim();
    } catch {
      return null;
    }
  },
  kill(pid) {
    try {
      process.kill(pid, 'SIGKILL');
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ESRCH') console.warn(`orphans: kill ${pid}: ${String(err)}`);
    }
  },
};

export function reapOrphans(dir: string, probe: Probe = realProbe): number[] {
  return listEntries(dir).filter((name) => reapOne(dir, name, probe)).map(Number);
}

function listEntries(dir: string): string[] {
  try {
    return readdirSync(dir).filter((name) => /^\d+$/.test(name));
  } catch {
    return [];
  }
}

interface PidRecord {
  owner: number;
  cmd: string;
}

function readRecord(file: string): PidRecord | null {
  try {
    const [owner, ...rest] = readFileSync(file, 'utf8').split('\n');
    const cmd = rest.join('\n').replace(/\n$/, '');
    return Number.isInteger(Number(owner)) && owner !== '' ? { owner: Number(owner), cmd } : null;
  } catch {
    return null;
  }
}

// Decision for one file; true — the process was killed.
function reapOne(dir: string, name: string, probe: Probe): boolean {
  const file = path.join(dir, name);
  const pid = Number(name);
  const rec = readRecord(file);
  if (rec === null) return drop(file);
  if (probe.alive(rec.owner)) return false;
  if (!probe.alive(pid)) return drop(file);
  if (probe.command(pid) !== rec.cmd) return drop(file);
  probe.kill(pid);
  drop(file);
  return true;
}

function drop(file: string): false {
  rmSync(file, { force: true });
  return false;
}
