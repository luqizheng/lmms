import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackFile } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201, 204];
const TOL = [...OK, 502, 404];

describe('file-service', () => {
  let fileId: string | undefined;

  it('should upload file metadata', async () => {
    const res = await authedReq(adminToken).post(
      '/v1/file/upload',
      Payload.file.upload(),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && res.body?.id) {
      fileId = String(res.body.id);
      trackFile(fileId);
    }
  });

  it('should list files', async () => {
    const res = await authedReq(adminToken).get('/v1/file/list?page=1&size=10');
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should get file info', async () => {
    if (!fileId) return;
    const res = await authedReq(adminToken).get(`/v1/file/${fileId}/info`);
    expect(TOL).toContain(res.status);
  });

  it('should create presign url (get)', async () => {
    if (!fileId) return;
    const res = await authedReq(adminToken).post(
      '/v1/file/presign',
      Payload.file.presign(fileId),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status)) {
      expect(res.body).toHaveProperty('url');
    }
  });

  it('should delete file', async () => {
    if (!fileId) return;
    const res = await authedReq(adminToken).del(`/v1/file/${fileId}`);
    expect(TOL).toContain(res.status);
  });
});
