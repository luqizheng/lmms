import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import request from 'supertest';
import { config } from '../shared/config';
import { resetBase } from '../shared/db/reset';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default async function () {
  await resetBase();

  // admin 由 API 注册（首个用户即 admin），保证密码哈希正确
  await request(config.baseURL).post('/v1/iam/auth/register').send({
    username: config.admin.user,
    password: config.admin.pass,
    nickname: config.admin.nickname,
    email: config.admin.email,
  });

  const login = await request(config.baseURL)
    .post('/v1/iam/auth/login')
    .send({ username: config.admin.user, password: config.admin.pass });
  if (login.status !== 200 || !login.body?.access_token) {
    throw new Error(`login admin failed: ${login.status} ${JSON.stringify(login.body)}`);
  }
  writeFileSync(
    join(__dirname, '.token.json'),
    JSON.stringify({ token: login.body.access_token }),
    'utf8',
  );
}
