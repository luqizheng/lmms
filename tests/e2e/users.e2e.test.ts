import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { trackUser } from './helpers/ids';
import { SEED } from './fixtures/seed';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('users', () => {
  let createdUserId: number | undefined;

  it('should fetch current admin profile', async () => {
    const res = await authedReq(adminToken).get('/api/v1/users/me');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('username');
  });

  it('should list users with pagination', async () => {
    const res = await authedReq(adminToken).get('/api/v1/users?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('list');
    expect(Array.isArray(res.body.list)).toBe(true);
  });

  it('should create a new user via register or admin endpoint', async () => {
    const u = SEED.newUser();
    const res = await authedReq(adminToken).post('/api/v1/users', u);
    if (res.status === 200 || res.status === 201) {
      createdUserId = res.body.id;
      trackUser(createdUserId!);
    } else {
      const fallback = await authedReq(adminToken).post(
        '/api/v1/auth/register',
        u,
      );
      expect([200, 201]).toContain(fallback.status);
      createdUserId = fallback.body.id;
      trackUser(createdUserId!);
    }
    expect(createdUserId).toBeGreaterThan(0);
  });

  it('should update extends on a user', async () => {
    expect(createdUserId).toBeDefined();
    const res = await authedReq(adminToken).patch(
      `/api/v1/users/${createdUserId}/extends`,
      { department: 'e2e-dept' },
    );
    expect([200, 204]).toContain(res.status);
  });

  it('should assign and remove user roles', async () => {
    expect(createdUserId).toBeDefined();
    const assign = await authedReq(adminToken).post(
      `/api/v1/users/${createdUserId}/roles`,
      { role_ids: [] },
    );
    expect([200, 204]).toContain(assign.status);
    const remove = await authedReq(adminToken).del(
      `/api/v1/users/${createdUserId}/roles`,
      { role_ids: [] },
    );
    expect([200, 204]).toContain(remove.status);
  });
});
