import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { queryUserByUsername } from './helpers/db';

const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:3000';

describe('User Registration, Login and Password Change', () => {
  let userId: string | undefined;
  let accessToken: string | undefined;

  it('should register first user as admin', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/register')
      .send({
        username: 'testadmin',
        password: 'testpass123',
        nickname: 'Test Admin',
        email: 'testadmin@example.com',
      });
    
    expect(res.status).toBe(201);
    expect(res.body).toHaveProperty('id');
    expect(res.body).toHaveProperty('username', 'testadmin');
    userId = res.body.id;

    const dbUser = await queryUserByUsername('testadmin');
    expect(dbUser).not.toBeNull();
    expect(dbUser?.username).toBe('testadmin');
    expect(dbUser?.email).toBe('testadmin@example.com');
    expect(dbUser?.nickname).toBe('Test Admin');
  });

  it('should login with registered user', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/login')
      .send({
        username: 'testadmin',
        password: 'testpass123',
      });
    
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('access_token');
    expect(res.body).toHaveProperty('token_type', 'Bearer');
    accessToken = res.body.access_token;
  });

  it('should change password', async () => {
    expect(accessToken).toBeDefined();
    
    const res = await request(baseURL)
      .put('/api/v1/auth/password')
      .set('Authorization', `Bearer ${accessToken}`)
      .send({
        old_password: 'testpass123',
        new_password: 'newpass456',
      });
    
    expect(res.status).toBe(200);
  });

  it('should login with new password', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/login')
      .send({
        username: 'testadmin',
        password: 'newpass456',
      });
    
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('access_token');
  });

  it('should fail login with old password', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/login')
      .send({
        username: 'testadmin',
        password: 'testpass123',
      });
    
    expect(res.status).toBe(401);
  });
});