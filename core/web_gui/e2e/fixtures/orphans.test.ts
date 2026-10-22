import { spawn } from 'node:child_process';
import { mkdtempSync, existsSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import * as path from 'node:path';

import { test, expect } from '@playwright/test';

import { forgetPid, realProbe, recordPid, reapOrphans } from './orphans';
import type { Probe } from './orphans';

const CMD = '/bin/target --port 1 --token x';

interface World {
  alive: Set<number>;
  commands: Map<number, string>;
  killed: number[];
}

function probeOf(w: World): Probe {
  return {
    alive: (pid) => w.alive.has(pid),
    command: (pid) => w.commands.get(pid) ?? null,
    kill: (pid) => void w.killed.push(pid),
  };
}

function setup(): { dir: string; file: (pid: number) => string } {
  const dir = mkdtempSync(path.join(tmpdir(), 'orphans-'));
  return { dir, file: (pid) => path.join(dir, String(pid)) };
}

test.describe('reapOrphans', () => {
  test('owner is dead, same line: killed, file removed', () => {
    const { dir, file } = setup();
    recordPid(dir, 500, 100, CMD);
    const w: World = { alive: new Set([500]), commands: new Map([[500, CMD]]), killed: [] };
    expect(reapOrphans(dir, probeOf(w))).toEqual([500]);
    expect(w.killed).toEqual([500]);
    expect(existsSync(file(500))).toBe(false);
    rmSync(dir, { recursive: true });
  });

  test('owner is alive (a live run with the same OUT): not killed, file stays', () => {
    const { dir, file } = setup();
    recordPid(dir, 500, 100, CMD);
    const w: World = { alive: new Set([500, 100]), commands: new Map([[500, CMD]]), killed: [] };
    expect(reapOrphans(dir, probeOf(w))).toEqual([]);
    expect(w.killed).toEqual([]);
    expect(existsSync(file(500))).toBe(true);
    rmSync(dir, { recursive: true });
  });

  test('pid taken by someone else (different line): not killed, file removed', () => {
    const { dir, file } = setup();
    recordPid(dir, 500, 100, CMD);
    const w: World = { alive: new Set([500]), commands: new Map([[500, '/MeshTown.app/meshtown']]), killed: [] };
    expect(reapOrphans(dir, probeOf(w))).toEqual([]);
    expect(w.killed).toEqual([]);
    expect(existsSync(file(500))).toBe(false);
    rmSync(dir, { recursive: true });
  });

  test('no such process: file removed', () => {
    const { dir, file } = setup();
    recordPid(dir, 500, 100, CMD);
    const w: World = { alive: new Set(), commands: new Map(), killed: [] };
    expect(reapOrphans(dir, probeOf(w))).toEqual([]);
    expect(w.killed).toEqual([]);
    expect(existsSync(file(500))).toBe(false);
    rmSync(dir, { recursive: true });
  });

  test('no directory: []', () => {
    const w: World = { alive: new Set(), commands: new Map(), killed: [] };
    expect(reapOrphans(path.join(tmpdir(), 'orphans-nonexistent-dir-xyz'), probeOf(w))).toEqual([]);
  });
});

test.describe('recordPid and forgetPid', () => {
  test('the file appears and disappears', () => {
    const { dir, file } = setup();
    recordPid(dir, 42, 7, CMD);
    expect(existsSync(file(42))).toBe(true);
    forgetPid(dir, 42);
    expect(existsSync(file(42))).toBe(false);
    rmSync(dir, { recursive: true });
  });
});

test.describe('cleanup: corrupted entry and a real process', () => {
  test('entry is empty or corrupted: file removed, not killed', () => {
    const { dir, file } = setup();
    writeFileSync(file(500), '');
    writeFileSync(file(501), 'not a number\nx\n');
    const w: World = { alive: new Set([500, 501]), commands: new Map([[500, CMD], [501, CMD]]), killed: [] };
    expect(reapOrphans(dir, probeOf(w))).toEqual([]);
    expect(w.killed).toEqual([]);
    expect(existsSync(file(500))).toBe(false);
    expect(existsSync(file(501))).toBe(false);
    rmSync(dir, { recursive: true });
  });

  test('realProbe.command of a real process equals the entry [bin, ...args].join', async () => {
    const { dir } = setup();
    const child = spawn('sleep', ['30'], { stdio: 'ignore' });
    const pid = child.pid as number;
    try {
      const cmd = ['sleep', '30'].join(' ');
      recordPid(dir, pid, process.pid, cmd);
      expect(realProbe.command(pid)).toBe(cmd);
    } finally {
      realProbe.kill(pid);
      rmSync(dir, { recursive: true });
    }
  });
});
