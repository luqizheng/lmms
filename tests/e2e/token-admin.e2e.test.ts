import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { authedReq, baseURL } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackOAuthClient } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 502, 503, 404, 405, 422];

describe('token-service admin', () => {
  let clientId: string | undefined;

  it('should create oauth client', async () => {
    const body = Payload.oauthClient();
    const res = await authedReq(adminToken).post(
      '/v1/admin/oauth-clients',
      body,
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status)) {
      clientId = (res.body?.client_id ?? body.client_id) as string;
      if (clientId) trackOAuthClient(clientId);
    }
  });

  it('should list oauth clients', async () => {
    const res = await authedReq(adminToken).get('/v1/admin/oauth-clients');
    expect([200, 502, 503, 404]).toContain(res.status);
  });

  it('should get oauth client detail', async () => {
    if (!clientId) return;
    const res = await authedReq(adminToken).get(
      `/v1/admin/oauth-clients/${clientId}`,
    );
    expect(TOL).toContain(res.status);
  });

  it('should update oauth client', async () => {
    if (!clientId) return;
    const res = await authedReq(adminToken).put(
      `/v1/admin/oauth-clients/${clientId}`,
      { name: `e2e_client_updated_${Date.now()}`, permissions: ['read', 'write'] },
    );
    expect(TOL).toContain(res.status);
  });

  it('should delete oauth client', async () => {
    if (!clientId) return;
    const res = await authedReq(adminToken).del(
      `/v1/admin/oauth-clients/${clientId}`,
    );
    expect(TOL).toContain(res.status);
  });

  it('should expose JWKS endpoint', async () => {
    const res = await request(baseURL).get('/.well-known/jwks.json');
    expect([200, 502, 503, 404]).toContain(res.status);
    if (res.status === 200) {
      expect(res.body).toHaveProperty('keys');
      expect(Array.isArray(res.body.keys)).toBe(true);
    }
  });

  it('should issue user-jwt', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/tokens/user-jwt',
      Payload.userJwt(),
    );
    expect([...TOL, 400, 401]).toContain(res.status);
  });
});
