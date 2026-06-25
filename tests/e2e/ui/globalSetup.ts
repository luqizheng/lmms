import { resetBase } from '../shared/db/reset';
import { config } from '../shared/config';
import { preflight } from '../shared/health';
import request from 'supertest';

export default async function () {
  await preflight();
  await resetBase();

  // admin 由 API 注册，保证密码哈希正确（UI 登录用同一账号）
  await request(config.baseURL).post('/v1/iam/auth/register').send({
    username: config.admin.user,
    password: config.admin.pass,
    nickname: config.admin.nickname,
    email: config.admin.email,
  });

  // 前端健康探测：失败即中止
  const res = await fetch(config.frontendURL).catch(() => null);
  if (!res || !res.ok) {
    throw new Error(`前端未就绪: ${config.frontendURL}`);
  }
}
