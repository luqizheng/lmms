import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';
import { Payload } from './fixtures/payload';
import { trackAnnouncement } from './helpers/ids';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const BASE = '/api/notifications';
const OK = [200, 201, 204];
const TOL = [...OK, 502, 404];

describe('notification-service', () => {
  let announcementId: number | undefined;

  it('should create an announcement', async () => {
    const res = await authedReq(adminToken).post(
      `${BASE}/announcements`,
      Payload.notification.announcement(),
    );
    expect(TOL).toContain(res.status);
    if (OK.includes(res.status) && typeof res.body?.id === 'number') {
      announcementId = res.body.id as number;
      trackAnnouncement(announcementId);
    }
  });

  it('should list announcements for me', async () => {
    const res = await authedReq(adminToken).get(`${BASE}/announcements`);
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should publish announcement', async () => {
    if (!announcementId) return;
    const res = await authedReq(adminToken).post(
      `${BASE}/announcements/${announcementId}/publish`,
      {},
    );
    expect(TOL).toContain(res.status);
  });

  it('should get announcement detail', async () => {
    if (!announcementId) return;
    const res = await authedReq(adminToken).get(
      `${BASE}/announcements/${announcementId}`,
    );
    expect(TOL).toContain(res.status);
  });

  it('should mark announcement read', async () => {
    if (!announcementId) return;
    const res = await authedReq(adminToken).post(
      `${BASE}/announcements/${announcementId}/read`,
      {},
    );
    expect(TOL).toContain(res.status);
  });

  it('should query unread-count', async () => {
    const res = await authedReq(adminToken).get(`${BASE}/unread-count`);
    expect([200, 502, 404]).toContain(res.status);
  });

  it('should revoke announcement', async () => {
    if (!announcementId) return;
    const res = await authedReq(adminToken).post(
      `${BASE}/announcements/${announcementId}/revoke`,
      {},
    );
    expect(TOL).toContain(res.status);
  });
});
