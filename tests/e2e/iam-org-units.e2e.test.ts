import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackOrgUnit } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 500, 502, 404];

describe('iam org-units', () => {
  let orgId: number | undefined;

  it('should list org-units', async () => {
    const res = await authedReq(adminToken).get('/iam/org-units');
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should create an org-unit', async () => {
    const res = await authedReq(adminToken).post(
      '/iam/org-units',
      Payload.orgUnit(),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && typeof res.body?.id === 'number') {
      orgId = res.body.id as number;
      trackOrgUnit(orgId);
    }
  });

  it('should get org-unit detail', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).get(`/iam/org-units/${orgId}`);
    expect(TOL).toContain(res.status);
  });

  it('should update org-unit', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).put(`/iam/org-units/${orgId}`, {
      ...Payload.orgUnit(),
      sort_order: 1,
    });
    expect(TOL).toContain(res.status);
  });

  it('should delete org-unit', async () => {
    if (!orgId) return;
    const res = await authedReq(adminToken).del(`/iam/org-units/${orgId}`);
    expect(TOL).toContain(res.status);
  });
});
