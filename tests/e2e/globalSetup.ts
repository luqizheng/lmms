import { login } from './helpers/auth';
import { SEED } from './fixtures/seed';
import request from 'supertest';
import { baseURL } from './helpers/http';

export default async function () {
  try {
    const token = await login(SEED.admin.user, SEED.admin.pass);
    (globalThis as unknown as { __adminToken__: string }).__adminToken__ = token;
  } catch (err) {
    console.log('Admin user not found, registering first admin...');
    await request(baseURL)
      .post('/api/v1/auth/register')
      .send({
        username: SEED.admin.user,
        password: SEED.admin.pass,
        nickname: 'E2E Admin',
        email: 'admin@test.local',
      });
    const token = await login(SEED.admin.user, SEED.admin.pass);
    (globalThis as unknown as { __adminToken__: string }).__adminToken__ = token;
  }
}