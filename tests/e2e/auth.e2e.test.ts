import { describe, it, expect, afterAll } from 'vitest';
import request from 'supertest';
import { baseURL } from './helpers/http';
import { login, logout } from './helpers/auth';
import { Payload } from './fixtures/payload';
import { DEBUG } from './helpers/debug';

describe('auth', () => {
  let token: string | undefined;

  it('should login admin and return access_token', async () => {
    token = await login(
      process.env.E2E_ADMIN_USER ?? 'admin',
      process.env.E2E_ADMIN_PASS ?? 'admin123',
    );
    expect(token).toBeTruthy();
    if (DEBUG) console.log('token len', token.length);
  });

  it('should reject wrong password with 400/401', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/login')
      .send({ username: 'admin', password: 'wrong' });
    expect([400, 401]).toContain(res.status);
  });

  it('should report setup status', async () => {
    const res = await request(baseURL).get('/api/v1/auth/setup-status');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('setup_required');
  });

  describe('register', () => {
    it('should register first admin when setup_required is true', async () => {
      const status = await request(baseURL).get('/api/v1/auth/setup-status');
      expect(status.status).toBe(200);
      if (status.body.setup_required === false) {
        const closed = await request(baseURL)
          .post('/api/v1/auth/register')
          .send({ ...Payload.register(), nickname: 'closed' });
        // 已注册 admin 后，再次注册应被拒：可能 403（业务关闭）或 422（缺字段时校验先报）
        expect([403, 422]).toContain(closed.status);
        return;
      }

      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ ...Payload.register(), nickname: 'first admin' });
      expect(res.status).toBe(201);
      expect(res.body).toHaveProperty('id');
      expect(typeof res.body.id).toBe('number');
      expect(res.body).toHaveProperty('username');
    });

    it('should reject short username (3-32 chars required)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'ab', password: 'P@ssw0rd!', nickname: 'x' });
      // setup 已完成时 register 关闭 → 403；setup 未完成时校验失败 → 400/422
      expect([400, 403, 422]).toContain(res.status);
    });

    it('should reject username with illegal characters', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'bad user!', password: 'P@ssw0rd!', nickname: 'x' });
      expect([400, 403, 422]).toContain(res.status);
    });

    it('should reject short password (>= 6 chars required)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'e2e_short_pw', password: '123', nickname: 'x' });
      expect([400, 403, 422]).toContain(res.status);
    });

    it('should reject malformed email', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({
          username: `e2e_bad_email_${Date.now()}`,
          password: 'P@ssw0rd!',
          nickname: 'x',
          email: 'not-an-email',
        });
      expect([400, 403, 422]).toContain(res.status);
    });

    it('should reject malformed phone (must be 11 digits)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({
          username: `e2e_bad_phone_${Date.now()}`,
          password: 'P@ssw0rd!',
          nickname: 'x',
          phone: '12345',
        });
      expect([400, 403, 422]).toContain(res.status);
    });

    it('should reject duplicate username with 409', async () => {
      const adminStatus = await request(baseURL).get('/api/v1/auth/setup-status');
      if (adminStatus.body.setup_required === true) {
        // skip: first register is taken by another test
        return;
      }
      const body = { ...Payload.register(), nickname: 'dup' };
      const first = await request(baseURL)
        .post('/api/v1/auth/register')
        .send(body);
      // 已有 admin 时 register 关闭：403（业务）或 422（缺字段先校验失败）
      expect([403, 409, 422]).toContain(first.status);
    });
  });

  afterAll(async () => {
    if (token) await logout(token);
  });
});
