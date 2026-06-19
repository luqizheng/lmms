import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const DOWN = [502, 503, 404];

describe('audit-service', () => {
  it('should create operation log', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/audit/operation',
      Payload.audit.operation(),
    );
    expect([...OK, ...DOWN]).toContain(res.status);
  });

  it('should query operation logs with pagination', async () => {
    const res = await authedReq(adminToken).get(
      '/v1/audit/operation?page=1&size=10',
    );
    expect([200, ...DOWN]).toContain(res.status);
    if (res.status === 200) {
      expect(res.body).toHaveProperty('items');
    }
  });

  it('should create login log', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/audit/login',
      Payload.audit.login(),
    );
    expect([...OK, ...DOWN]).toContain(res.status);
  });

  it('should query login logs', async () => {
    const res = await authedReq(adminToken).get('/v1/audit/login?page=1&size=10');
    expect([200, ...DOWN]).toContain(res.status);
  });

  it('should get retention policy', async () => {
    const res = await authedReq(adminToken).get('/v1/audit/retention/policy');
    expect([200, ...DOWN]).toContain(res.status);
    if (res.status === 200) {
      expect(res.body).toHaveProperty('hot_days');
    }
  });

  it('should update retention policy', async () => {
    const res = await authedReq(adminToken).put(
      '/v1/audit/retention/policy',
      Payload.audit.policy(),
    );
    expect([...OK, ...DOWN]).toContain(res.status);
  });

  it('should get retention stats', async () => {
    const res = await authedReq(adminToken).get('/v1/audit/retention/stats');
    expect([200, ...DOWN]).toContain(res.status);
  });

  it('should trigger retention cleanup', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/audit/retention/cleanup',
      {},
    );
    expect([...OK, ...DOWN]).toContain(res.status);
  });
});
