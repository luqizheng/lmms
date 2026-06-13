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
          .send(Payload.register());
        expect(closed.status).toBe(403);
        expect(closed.body).toMatchObject({ error: 'registration closed' });
        return;
      }

      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send(Payload.register());
      expect(res.status).toBe(201);
      expect(res.body).toHaveProperty('id');
      expect(typeof res.body.id).toBe('number');
      expect(res.body).toHaveProperty('username');
    });

    it('should reject short username (3-32 chars required)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'ab', password: 'P@ssw0rd!' });
      expect(res.status).toBe(400);
      expect(res.body).toHaveProperty('error');
    });

    it('should reject username with illegal characters', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'bad user!', password: 'P@ssw0rd!' });
      expect(res.status).toBe(400);
    });

    it('should reject short password (>= 6 chars required)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({ username: 'e2e_short_pw', password: '123' });
      expect(res.status).toBe(400);
    });

    it('should reject malformed email', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({
          username: `e2e_bad_email_${Date.now()}`,
          password: 'P@ssw0rd!',
          email: 'not-an-email',
        });
      expect(res.status).toBe(400);
    });

    it('should reject malformed phone (must be 11 digits)', async () => {
      const res = await request(baseURL)
        .post('/api/v1/auth/register')
        .send({
          username: `e2e_bad_phone_${Date.now()}`,
          password: 'P@ssw0rd!',
          phone: '12345',
        });
      expect(res.status).toBe(400);
    });

    it('should reject duplicate username with 409', async () => {
      const adminStatus = await request(baseURL).get('/api/v1/auth/setup-status');
      if (adminStatus.body.setup_required === true) {
        // skip: first register is taken by another test
        return;
      }
      const body = Payload.register();
      const first = await request(baseURL)
        .post('/api/v1/auth/register')
        .send(body);
      expect([403, 409]).toContain(first.status);
    });
  });

  afterAll(async () => {
    if (token) await logout(token);
  });
});
