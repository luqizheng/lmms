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

  it('should register a new user', async () => {
    const body = Payload.register();
    const res = await request(baseURL).post('/api/v1/auth/register').send(body);
    expect([200, 201]).toContain(res.status);
    expect(res.body).toHaveProperty('id');
  });

  it('should report setup status', async () => {
    const res = await request(baseURL).get('/api/v1/auth/setup-status');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('setup_required');
  });

  afterAll(async () => {
    if (token) await logout(token);
  });
});
