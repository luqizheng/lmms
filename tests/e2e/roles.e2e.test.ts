import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { trackRole } from './helpers/ids';
import { Payload } from './fixtures/payload';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('roles', () => {
  let createdRoleId: number | undefined;

  it('should list roles with pagination', async () => {
    const res = await authedReq(adminToken).get('/roles?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('list');
  });

  it('should create a role', async () => {
    const res = await authedReq(adminToken).post('/roles', Payload.role());
    expect([200, 201]).toContain(res.status);
    expect(typeof res.body.id).toBe('number');
    createdRoleId = res.body.id as number;
    trackRole(createdRoleId);
  });

  it('should fetch a role by id', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).get(`/roles/${createdRoleId}`);
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('code');
  });

  it('should update a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken)
      .put(`/roles/${createdRoleId}`, { ...Payload.role(), status: 1 });
    expect([200, 204]).toContain(res.status);
  });

  it('should assign permissions to a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).post(
      `/roles/${createdRoleId}/permissions`,
      { permission_ids: [] },
    );
    expect([200, 204]).toContain(res.status);
  });

  it('should delete a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).del(`/roles/${createdRoleId}`);
    expect([200, 204]).toContain(res.status);
  });
});
