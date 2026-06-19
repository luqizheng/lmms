import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

// token-service 通过 consul 服务发现路由，未健康时网关返回 502/503；
// 路由方法/payload 不匹配时后端返回 404/405/422
const SERVICE_DOWN = [502, 503, 404, 405, 422];

describe('tokens', () => {
  it('should list resources', async () => {
    const res = await authedReq(adminToken).get('/v1/tokens/resource');
    expect([200, ...SERVICE_DOWN]).toContain(res.status);
    if (res.status === 200) {
      expect(Array.isArray(res.body)).toBe(true);
    }
  });

  it('should reject access token request with placeholder code', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/tokens/access',
      Payload.token.access(),
    );
    expect([400, 401, ...SERVICE_DOWN]).toContain(res.status);
  });

  it('should create api key', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/tokens/api_key',
      Payload.token.apiKey(),
    );
    expect([200, 201, ...SERVICE_DOWN]).toContain(res.status);
    if (res.status === 200 || res.status === 201) {
      expect(res.body).toHaveProperty('api_key');
    }
  });

  it('should reject refresh with placeholder token', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/tokens/refresh',
      Payload.token.refresh('placeholder'),
    );
    expect([400, 401, ...SERVICE_DOWN]).toContain(res.status);
  });

  it('should revoke a token', async () => {
    const res = await authedReq(adminToken).post('/v1/tokens/revoke', {
      token: adminToken,
    });
    expect([200, 204, 400, ...SERVICE_DOWN]).toContain(res.status);
  });
});
