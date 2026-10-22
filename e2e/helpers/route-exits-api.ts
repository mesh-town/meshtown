// Reads and writes `route.exits` through the target's HTTP API (PUT and GET `/api/config`, field `route.exits`).
import type { MeshApi } from '../fixtures/api';
import { expect } from '../fixtures/index';

// An unknown id: no node with such an id is in the network (checks the refusal for an unknown id).
export const STRANGER_ID = '8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c';
export const STRANGER_LABEL = STRANGER_ID.slice(0, 10);
export const POLL_TIMEOUT_MS = 6000;

export interface Peer {
  id: string;
  id_full: string;
  name?: string;
  self?: boolean;
  exit?: boolean;
  online?: boolean;
}
export interface StateDoc {
  route?: { state?: string };
  peers?: Peer[];
}
export interface ConfigDoc {
  rev?: number;
  config?: { route?: { exits?: string[] } } & Record<string, unknown>;
}

export async function configExits(api: MeshApi): Promise<string[] | undefined> {
  const { status, body } = await api.getJson<ConfigDoc>('/api/config');
  return status === 200 ? body?.config?.route?.exits : undefined;
}

export async function routeState(api: MeshApi): Promise<string | undefined> {
  const { status, body } = await api.getJson<StateDoc>('/api/state');
  return status === 200 ? body?.route?.state : undefined;
}

// GET the document, swap `route.exits`, PUT the whole document with `rev` — as the page does.
export async function putRouteExits(api: MeshApi, exits: string[]): Promise<{ status: number; body: unknown }> {
  const cfg = await api.getJson<ConfigDoc>('/api/config');
  if (cfg.status !== 200 || !cfg.body) {
    return { status: cfg.status, body: cfg.body };
  }
  const doc = structuredClone(cfg.body.config ?? {});
  doc.route = { ...doc.route, exits };
  return api.putJson('/api/config', { rev: cfg.body.rev, config: doc });
}

export async function expectConfigExits(api: MeshApi, expected: string[]): Promise<void> {
  await expect.poll(() => configExits(api), { timeout: POLL_TIMEOUT_MS }).toEqual(expected);
}
