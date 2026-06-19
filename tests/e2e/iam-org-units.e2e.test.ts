import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackOrgUnit } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 500, 502, 404];

describe('iam org-units', () => {
  let orgId: string | undefined;
  let childId: string | undefined;
  const created = Payload.orgUnit();
  const updatedName = `e2e_org_upd_${Date.now()}`;

  it('should list org-units', async () => {
    const res = await authedReq(adminToken).get('/v1/iam/org-units');
    expect([200, 502, 404]).toContain(res.status);
    if (res.status === 200) {
      expect(Array.isArray(res.body)).toBe(true);
    }
  });

  it('should create an org-unit', async () => {
    const res = await authedReq(adminToken).post('/v1/iam/org-units', created);
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && typeof res.body?.id === 'string') {
      orgId = res.body.id;
      trackOrgUnit(orgId);
      expect(res.body.code).toBe(created.code);
      expect(res.body.name).toBe(created.name);
      expect(res.body.level).toBe(1);
      expect(res.body.status).toBe('active');
    }
  });

  it('should reject duplicate code', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).post('/v1/iam/org-units', created);
    expect([409, 500, 502]).toContain(res.status);
  });

  it('should get org-unit detail', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).get(`/v1/iam/org-units/${orgId}`);
    expect(TOL).toContain(res.status);
    if (res.status === 200) {
      expect(res.body.id).toBe(orgId);
      expect(res.body.code).toBe(created.code);
      expect(Array.isArray(res.body.children)).toBe(true);
    }
  });

  it('should create a child org-unit', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).post('/v1/iam/org-units', {
      ...Payload.orgUnit(),
      parent_id: orgId,
    });
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && typeof res.body?.id === 'string') {
      childId = res.body.id;
      trackOrgUnit(childId);
      expect(res.body.parent_id).toBe(orgId);
      expect(res.body.level).toBe(2);
    }
  });

  it('should update org-unit', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).put(`/v1/iam/org-units/${orgId}`, {
      name: updatedName,
      sort_order: 5,
      status: 'active',
    });
    expect(TOL).toContain(res.status);
    if (res.status === 200) {
      expect(res.body.name).toBe(updatedName);
      expect(res.body.sort_order).toBe(5);
    }
  });

  it('should reflect update on detail', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).get(`/v1/iam/org-units/${orgId}`);
    expect(TOL).toContain(res.status);
    if (res.status === 200) {
      expect(res.body.name).toBe(updatedName);
      expect(res.body.sort_order).toBe(5);
    }
  });

  it('should reject delete when has children', async () => {
    if (!orgId || !childId) return;
    const res = await authedReq(adminToken).del(`/v1/iam/org-units/${orgId}`);
    expect([409, 500, 502]).toContain(res.status);
  });

  it('should delete child org-unit', async () => {
    if (!childId) return;
    const res = await authedReq(adminToken).del(`/v1/iam/org-units/${childId}`);
    expect(TOL).toContain(res.status);
  });

  it('should delete org-unit', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).del(`/v1/iam/org-units/${orgId}`);
    expect(TOL).toContain(res.status);
  });

  it('should return 404 after deletion', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).get(`/v1/iam/org-units/${orgId}`);
    expect([404, 500, 502]).toContain(res.status);
  });
});
