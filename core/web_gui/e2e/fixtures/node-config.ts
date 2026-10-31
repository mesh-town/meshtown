// Config and key of the node (`meshtown`) for the node target flags.
// Split out of `target-launch.ts` per the file-size rule — together with it the node-target file
// exceeded the 300-line ceiling.
import { execFile } from 'node:child_process';
import { chmod, writeFile } from 'node:fs/promises';
import * as path from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

export type NodeFileKind = 'asis' | 'readonly' | 'invalid' | 'unreadable' | 'field-error' | 'no-key';

export interface NodePlan {
  fileKind: NodeFileKind;
  socksTail: readonly string[];
}

// The `NODE_EXTRA_ARGS_MAP` table determines, from a case's extraArgs, the shape of the node's config.json and the tail
// of argv about socks. The key is the JSON of the normalized array: cases' extraArgs contain no nested
// structures, so `JSON.stringify` gives an unambiguous comparison of arrays as strings.
const NODE_EXTRA_ARGS_MAP: ReadonlyMap<string, NodePlan> = new Map([
  [nodePlanKey([]), { fileKind: 'asis', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--cli-override', 'node.name']), { fileKind: 'asis', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--file-state', 'readonly']), { fileKind: 'readonly', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--file-state', 'invalid']), { fileKind: 'invalid', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--file-state', 'unreadable']), { fileKind: 'unreadable', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--field-error', 'socks.port']), { fileKind: 'field-error', socksTail: ['--no-socks'] }],
  [nodePlanKey(['--no-network']), { fileKind: 'no-key', socksTail: ['--no-socks'] }],
]);

function nodePlanKey(extraArgs: readonly string[]): string {
  return JSON.stringify(extraArgs);
}

// `socksPort` — overrides socks.port with a free port (`TargetFlags.socksOverride`): without
// it the table holds the hard-coded port 2000 (`--cli-override socks.port=2000`).
export function resolveNodePlan(extraArgs: readonly string[], socksPort?: number): NodePlan {
  const plan = NODE_EXTRA_ARGS_MAP.get(nodePlanKey(extraArgs));
  if (plan === undefined) {
    throw new Error(`unknown extraArgs for the node target: ${JSON.stringify(extraArgs)}`);
  }
  if (socksPort === undefined) return plan;
  return { ...plan, socksTail: ['--socks-port', String(socksPort)] };
}

// 32 zero bytes — the same trick as in the `config_file.rs` tests (`assert_eq!(k.to_bytes(), [0u8;
// 32])`): a valid node identity secret, so that `config_file::prepare` does not treat the identity as "fresh"
// and does not rewrite the file at startup (otherwise `chmod 0o444` would be undone in the node's first second of life).
const ZERO_SECRET_BASE64 = Buffer.alloc(32).toString('base64');

// `network create --key-only` roots the key at the identity of the config file and refuses a file without one; a valid secret in every JSON
// view of the file also keeps `prepare` from rewriting it, so the field errors and the read-only file stay as written.
const IDENTITY = { secret: ZERO_SECRET_BASE64 };

// e2e does not listen on the local network: `web.access` defaults to `localnet`.
const WEB_LOCAL = { access: 'localhost' };

function nodeConfigText(fileKind: NodeFileKind): string {
  switch (fileKind) {
    case 'invalid':
      return '{not json';
    case 'field-error':
      return JSON.stringify({ identity: IDENTITY, config: { version: 1, web: WEB_LOCAL, socks: { port: 80 } } });
    case 'readonly':
      return JSON.stringify({ identity: IDENTITY, config: { version: 1, web: WEB_LOCAL } });
    case 'asis':
    case 'unreadable':
    case 'no-key':
      return JSON.stringify({ identity: IDENTITY, config: { version: 1, web: WEB_LOCAL } });
  }
}

export async function prepareNodeConfig(dir: string, fileKind: NodeFileKind): Promise<string> {
  const configPath = path.join(dir, 'config.json');
  await writeFile(configPath, nodeConfigText(fileKind));
  if (fileKind === 'readonly') await chmod(configPath, 0o444);
  if (fileKind === 'unreadable') await chmod(configPath, 0o000);
  return configPath;
}

// The config file `network create --key-only` reads. `invalid` and `unreadable` files cannot be read by it, so it gets a small valid file of its own
// (the same identity): the key only has to be well formed, the node under test does not need to be its root.
export async function keyConfigPath(dir: string, fileKind: NodeFileKind, configPath: string): Promise<string> {
  if (fileKind !== 'invalid' && fileKind !== 'unreadable') return configPath;
  const own = path.join(dir, 'key-identity.json');
  await writeFile(own, JSON.stringify({ identity: IDENTITY, config: { version: 1 } }));
  return own;
}

export async function nodeNewKey(bin: string, configPath: string): Promise<string> {
  try {
    const { stdout } = await execFileAsync(bin, ['network', 'create', '--key-only', '--raw', '--config', configPath]);
    return stdout.trim();
  } catch (err) {
    throw new Error(`meshtown network create --key-only --raw: ${execErrorMessage(err)}`);
  }
}

function execErrorMessage(err: unknown): string {
  if (err !== null && typeof err === 'object' && 'stderr' in err) {
    return String((err as { stderr: unknown }).stderr);
  }
  return String(err);
}

// The key is always passed with the `--key` flag, except for `no-key`: the node starts with no key at all
// (`--no-network`). Making the key still runs in that case,
// and `buildNodeArgs` simply does not add `--key`.
export function buildNodeArgs(key: string, configPath: string, port: number, plan: NodePlan): string[] {
  const keyArgs = plan.fileKind === 'no-key' ? [] : ['--key', key];
  return [
    'run',
    ...keyArgs,
    '--config',
    configPath,
    '--web-port',
    String(port),
    '--set',
    'checks.enabled=false',
    ...plan.socksTail,
  ];
}
