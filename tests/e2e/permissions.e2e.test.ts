import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('permissions', () => {
  it('should list all permissions', async () => {
    const res = await authedReq(adminToken).get('/permissions');
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
  });
});
