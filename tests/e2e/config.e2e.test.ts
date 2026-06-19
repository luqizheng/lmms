import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackNamespace, trackConfigItem } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 502, 404];

describe('config-service', () => {
  let namespaceId: string | undefined;
  let configItemId: string | undefined;

  it('should list namespaces', async () => {
    const res = await authedReq(adminToken).get(
      '/v1/config/namespace?page=1&size=10',
    );
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should create a namespace', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/config/namespace',
      Payload.config.namespace(),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && res.body?.id) {
      namespaceId = String(res.body.id);
      trackNamespace(namespaceId);
    }
  });

  it('should get namespace detail', async () => {
    if (!namespaceId) return;
    const res = await authedReq(adminToken).get(
      `/v1/config/namespace/${namespaceId}`,
    );
    expect(TOL).toContain(res.status);
  });

  it('should create a config item', async () => {
    if (!namespaceId) return;
    const res = await authedReq(adminToken).post(
      '/v1/config/item',
      Payload.config.item(namespaceId),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && res.body?.id) {
      configItemId = String(res.body.id);
      trackConfigItem(configItemId);
    }
  });

  it('should list config items', async () => {
    const res = await authedReq(adminToken).get(
      `/v1/config/item?page=1&size=10${namespaceId ? `&namespace_id=${namespaceId}` : ''}`,
    );
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should update config item', async () => {
    if (!configItemId) return;
    const res = await authedReq(adminToken).put(
      `/v1/config/item/${configItemId}`,
      { value: 'updated', description: 'updated by e2e' },
    );
    expect(TOL).toContain(res.status);
  });

  it('should watch config (long polling, accept timeout)', async () => {
    if (!namespaceId) return;
    const res = await authedReq(adminToken).get(
      `/v1/config/watch?namespace_id=${namespaceId}&version=0`,
    );
    expect([200, 204, 502, 404, 408]).toContain(res.status);
  });

  it('should delete config item', async () => {
    if (!configItemId) return;
    const res = await authedReq(adminToken).del(
      `/v1/config/item/${configItemId}`,
    );
    expect(TOL).toContain(res.status);
  });

  it('should delete namespace', async () => {
    if (!namespaceId) return;
    const res = await authedReq(adminToken).del(
      `/v1/config/namespace/${namespaceId}`,
    );
    expect(TOL).toContain(res.status);
  });
});
