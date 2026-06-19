import { describe, expect, it } from 'vitest';
import { authedReq } from './helpers/http';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201];
const stamp = Date.now();

const routesJson = [
  {
    path: `/e2e-menu-${stamp}`,
    name: `E2eMenu${stamp}`,
    component: 'system/menu/index',
    meta: {
      title: 'E2E 菜单',
      auths: ['e2e:menu:list'],
      rank: 99,
    },
    _menu: {
      id: 900001,
      parentId: 0,
      menuType: 0,
      auths: 'e2e:menu:list',
    },
  },
];

describe('menu configs', () => {
  let configId: string | undefined;

  it('should list menu configs', async () => {
    const res = await authedReq(adminToken).get('/v1/pure/menu-configs');
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data).toHaveProperty('items');
  });

  it('should create a draft menu config', async () => {
    const res = await authedReq(adminToken).post('/v1/pure/menu-configs', {
      name: `e2e menu config ${stamp}`,
      description: 'created by e2e',
      routes_json: routesJson,
      created_by: 'e2e',
    });

    expect(OK).toContain(res.status);
    expect(res.body.code).toBe(0);
    expect(res.body.data.status).toBe('draft');
    expect(res.body.data.routes_json).toEqual(routesJson);
    configId = res.body.data.id;
  });

  it('should get menu config detail', async () => {
    if (!configId) return;
    const res = await authedReq(adminToken).get(`/v1/pure/menu-configs/${configId}`);
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.id).toBe(configId);
  });

  it('should update routes_json', async () => {
    if (!configId) return;
    const updatedRoutes = [
      {
        ...routesJson[0],
        meta: {
          ...routesJson[0].meta,
          title: 'E2E 菜单已更新',
        },
      },
    ];

    const res = await authedReq(adminToken).put(`/v1/pure/menu-configs/${configId}`, {
      routes_json: updatedRoutes,
    });

    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.routes_json).toEqual(updatedRoutes);
  });

  it('should publish menu config', async () => {
    if (!configId) return;
    const res = await authedReq(adminToken).post(`/v1/pure/menu-configs/${configId}/publish`);
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.status).toBe('published');
    expect(res.body.data.is_active).toBe(true);
  });

  it('should expose published routes from get-async-routes', async () => {
    const res = await authedReq(adminToken).get('/v1/pure/get-async-routes');
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(Array.isArray(res.body.data)).toBe(true);
    expect(JSON.stringify(res.body.data)).toContain('E2E 菜单已更新');
  });
});
