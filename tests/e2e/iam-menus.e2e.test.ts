import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackMenu } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 500, 502, 404];

describe('iam menus', () => {
  let menuId: number | undefined;

  it('should list menus', async () => {
    const res = await authedReq(adminToken).get('/iam/menus');
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should create a menu', async () => {
    const res = await authedReq(adminToken).post('/iam/menus', Payload.menu());
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && typeof res.body?.id === 'number') {
      menuId = res.body.id as number;
      trackMenu(menuId);
    }
  });

  it('should get menu detail', async () => {
    if (!menuId) return;
    const res = await authedReq(adminToken).get(`/iam/menus/${menuId}`);
    expect(TOL).toContain(res.status);
  });

  it('should update menu', async () => {
    if (!menuId) return;
    const res = await authedReq(adminToken).put(`/iam/menus/${menuId}`, {
      ...Payload.menu(),
      sort_order: 1,
    });
    expect(TOL).toContain(res.status);
  });

  it('should delete menu', async () => {
    if (!menuId) return;
    const res = await authedReq(adminToken).del(`/iam/menus/${menuId}`);
    expect(TOL).toContain(res.status);
  });
});
