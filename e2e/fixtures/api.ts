// HTTP API client of the target — methods and response parsing.
import type { APIRequestContext } from '@playwright/test';

import { test as targetTest } from './target';
import type { RunningTarget } from './target';

export interface RawRequest {
  method: 'GET' | 'PUT';
  path: string;
  headers?: Record<string, string>;
  data?: unknown;
}

export interface ApiReply<T> {
  status: number;
  body: T | null;
}

export interface ConfigDoc {
  config?: { node?: { name?: string } };
}

export class MeshApi {
  constructor(
    private readonly request: APIRequestContext,
    private readonly target: RunningTarget,
  ) {}

  // Exactly the given headers and body, adds no token — the guards test checks the protection itself.
  async status(req: RawRequest): Promise<number> {
    const res = await this.request.fetch(this.target.origin + req.path, {
      method: req.method,
      headers: req.headers,
      data: req.data,
      failOnStatusCode: false,
      maxRedirects: 0,
    });
    return res.status();
  }

  // Like target_api_get: token in the query string, header Host: target.host; an empty body → null.
  async getJson<T>(path: string): Promise<ApiReply<T>> {
    const sep = path.includes('?') ? '&' : '?';
    const url = `${this.target.origin}${path}${sep}token=${this.target.token}`;
    const res = await this.request.fetch(url, {
      method: 'GET',
      headers: { Host: this.target.host },
      failOnStatusCode: false,
      maxRedirects: 0,
    });
    const text = await res.text();
    const body = text === '' ? null : (JSON.parse(text) as T);
    return { status: res.status(), body };
  }

  // Like target_api_put: token in the query string, the target's Host and Origin, JSON body.
  async putJson<T>(path: string, data: unknown): Promise<ApiReply<T>> {
    const sep = path.includes('?') ? '&' : '?';
    const url = `${this.target.origin}${path}${sep}token=${this.target.token}`;
    const res = await this.request.fetch(url, {
      method: 'PUT',
      headers: { Host: this.target.host, Origin: this.target.origin },
      data,
      failOnStatusCode: false,
      maxRedirects: 0,
    });
    const text = await res.text();
    const body = text === '' ? null : (JSON.parse(text) as T);
    return { status: res.status(), body };
  }
}

export const test = targetTest.extend<{ api: MeshApi }>({
  api: async ({ request, target }, use) => {
    await use(new MeshApi(request, target));
  },
});
