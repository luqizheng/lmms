import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import request from 'supertest';
import { baseURL } from './helpers/http';
import { clearUsers } from './helpers/db';

const __dirname = dirname(fileURLToPath(import.meta.url));

const ADMIN_USER = process.env.E2E_ADMIN_USER ?? 'e2eadmin';
const ADMIN_PASS = process.env.E2E_ADMIN_PASS ?? 'P@ssw0rd!';
const ADMIN_NICK = 'E2E Admin';
const ADMIN_EMAIL = 'e2eadmin@test.local';

export default async function () {
  // 1. 清空用户相关数据
  await clearUsers();

  // 2. 注册测试 admin（首个用户即 admin）
  const reg = await request(baseURL)
    .post('/v1/iam/auth/register')
    .send({
      username: ADMIN_USER,
      password: ADMIN_PASS,
      nickname: ADMIN_NICK,
      email: ADMIN_EMAIL,
    });
  if (reg.status !== 201 && reg.status !== 200) {
    throw new Error(
      `register e2e admin failed: ${reg.status} ${JSON.stringify(reg.body)}`,
    );
  }

  // 3. 登录拿 access_token
  const login = await request(baseURL)
    .post('/v1/iam/auth/login')
    .send({ username: ADMIN_USER, password: ADMIN_PASS });
  if (login.status !== 200 || !login.body?.access_token) {
    throw new Error(
      `login e2e admin failed: ${login.status} ${JSON.stringify(login.body)}`,
    );
  }

  // 4. 写缓存文件，供 setupFiles 读取
  const cache = {
    token: login.body.access_token as string,
    username: ADMIN_USER,
    password: ADMIN_PASS,
  };
  writeFileSync(
    join(__dirname, '.e2e-admin.json'),
    JSON.stringify(cache),
    'utf8',
  );

  process.env.E2E_ADMIN_USER = ADMIN_USER;
  process.env.E2E_ADMIN_PASS = ADMIN_PASS;
}
