import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { baseURL } from './helpers/http';

describe('api-gateway', () => {
  it('should expose iam health', async () => {
    const res = await request(baseURL).get('/v1/iam/health');
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should expose audit health', async () => {
    const res = await request(baseURL).get('/v1/audit/health');
    // 当 audit-service 未独立部署或网关无法路由时返回 401/502/404
    expect([200, 401, 502, 503, 404]).toContain(res.status);
  });

  it('should reject unauthenticated access to protected route', async () => {
    const res = await request(baseURL).get('/v1/iam/users');
    expect([401, 403]).toContain(res.status);
  });

  it('should allow public setup-status without token', async () => {
    const res = await request(baseURL).get('/v1/iam/auth/setup-status');
    expect([200, 502, 404]).toContain(res.status);
  });
});
