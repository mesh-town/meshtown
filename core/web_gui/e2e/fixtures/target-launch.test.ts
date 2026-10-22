import { test, expect } from '@playwright/test';
import { resolveNodePlan } from './node-config';

// Table of the node's extra arguments (`NODE_EXTRA_ARGS_MAP`) —
// each of the 8 entries is checked by its own test, plus one deliberately unknown tuple.
test.describe('resolveNodePlan', () => {
  test('[] — asis, --no-socks', () => {
    expect(resolveNodePlan([])).toEqual({ fileKind: 'asis', socksTail: ['--no-socks'] });
  });

  test('["--cli-override", "node.name"] — asis, --no-socks', () => {
    expect(resolveNodePlan(['--cli-override', 'node.name'])).toEqual({
      fileKind: 'asis',
      socksTail: ['--no-socks'],
    });
  });

  test('["--file-state", "readonly"] — readonly, --no-socks', () => {
    expect(resolveNodePlan(['--file-state', 'readonly'])).toEqual({
      fileKind: 'readonly',
      socksTail: ['--no-socks'],
    });
  });

  test('["--file-state", "invalid"] — invalid, --no-socks', () => {
    expect(resolveNodePlan(['--file-state', 'invalid'])).toEqual({
      fileKind: 'invalid',
      socksTail: ['--no-socks'],
    });
  });

  test('["--file-state", "unreadable"] — unreadable, --no-socks', () => {
    expect(resolveNodePlan(['--file-state', 'unreadable'])).toEqual({
      fileKind: 'unreadable',
      socksTail: ['--no-socks'],
    });
  });

  test('["--field-error", "socks.port"] — field-error, --no-socks', () => {
    expect(resolveNodePlan(['--field-error', 'socks.port'])).toEqual({
      fileKind: 'field-error',
      socksTail: ['--no-socks'],
    });
  });

  test('[] with a free socksPort — asis, --socks-port <port>', () => {
    expect(resolveNodePlan([], 41234)).toEqual({
      fileKind: 'asis',
      socksTail: ['--socks-port', '41234'],
    });
  });

  test('["--no-network"] — no-key, --no-socks', () => {
    expect(resolveNodePlan(['--no-network'])).toEqual({ fileKind: 'no-key', socksTail: ['--no-socks'] });
  });

  test('unknown tuple — error', () => {
    expect(() => resolveNodePlan(['--unknown-flag'])).toThrow('unknown extraArgs for the node target');
  });
});
