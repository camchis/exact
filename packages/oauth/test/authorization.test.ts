import { afterEach, beforeEach, expect, test } from 'bun:test';
import { oauth, type OAuthConfig, type OAuthStore } from '../index.ts';

const callback = 'com.example.app:/oauth';
const config: OAuthConfig = {
  issuer: 'https://login.example.com', clientId: 'client', key: 'authorization.session', scopes: [], apis: [], callback,
  endpoints: { authorization: 'https://login.example.com/authorize', token: 'https://login.example.com/token' },
  fetch: () => { throw new Error('No token request should be sent'); },
};
const memory = (): OAuthStore => {
  const values = new Map<string, string>();
  return { get: key => values.get(key) ?? null, set: (key, value) => { values.set(key, value); }, forget: key => { values.delete(key); } };
};
const globals = globalThis as Record<string, unknown>;
let saved: PropertyDescriptor | undefined;
beforeEach(() => { saved = Object.getOwnPropertyDescriptor(globalThis, 'openAuthSession'); });
afterEach(() => {
  if (saved) Object.defineProperty(globalThis, 'openAuthSession', saved);
  else delete globals.openAuthSession;
});

test('a failed system auth session keeps its status and reason', async () => {
  for (const message of ['the session could not start (no window)', 'The browser ended the session.']) {
    globals.openAuthSession = () => Promise.reject(Object.assign(new Error(message), { status: 502 }));
    const provider = oauth(config), store = memory();
    await provider.session(store);
    expect(await provider.signIn(store)).toEqual({
      ok: false, code: 'auth_session', error: `Sign-in could not finish: ${message}`, retryable: false, status: 502,
    });
    expect(store.get(config.key)).toBeNull();
  }
});
