import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { trackUser } from './helpers/ids';
import { SEED } from './fixtures/seed';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('users', () => {
  let createdUserId: number | undefined;

  it('should list users with pagination', async () => {
    const res = await authedReq(adminToken).get('/v1/iam/users?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('items');
    expect(Array.isArray(res.body.items)).toBe(true);
  });

  it('should create a new user via admin endpoint', async () => {
    const u = SEED.newUser();
    const res = await authedReq(adminToken).post('/v1/iam/users', u);
    expect([200, 201]).toContain(res.status);
    createdUserId = res.body.id;
    trackUser(createdUserId!);
    expect(createdUserId).toBeDefined();
  });

  it('should fetch user detail by id', async () => {
    expect(createdUserId).toBeDefined();
    const res = await authedReq(adminToken).get(`/v1/iam/users/${createdUserId}`);
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('username');
  });

  it('should update user nickname', async () => {
    expect(createdUserId).toBeDefined();
    const res = await authedReq(adminToken).put(
      `/v1/iam/users/${createdUserId}`,
      { nickname: 'updated' },
    );
    expect([200, 204]).toContain(res.status);
  });

  it('should delete the user', async () => {
    expect(createdUserId).toBeDefined();
    const res = await authedReq(adminToken).del(`/v1/iam/users/${createdUserId}`);
    expect([200, 204]).toContain(res.status);
  });
});
