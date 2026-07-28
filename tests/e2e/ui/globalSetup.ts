import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';
import { createConnection } from 'mysql2/promise';
import { resetBase } from '../shared/db/reset';
import { config } from '../shared/config';
import { preflight } from '../shared/health';
import request from 'supertest';

const __dirname = dirname(fileURLToPath(import.meta.url));

const nid = () => {
  const alpha = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-';
  let out = '';
  for (let i = 0; i < 21; i++) out += alpha[Math.floor(Math.random() * alpha.length)];
  return out;
};

// resetBase 会清空 iam_permissions；角色分配权限等 UI 依赖权限树非空，
// 与 scripts/demo-data.js 同源，从 contracts/permissions.yaml 重灌。
async function seedPermissions(): Promise<number> {
  const yamlPath = join(__dirname, '..', '..', '..', 'contracts', 'permissions.yaml');
  const doc = parse(readFileSync(yamlPath, 'utf8'), { uniqueKeys: false }) as { permissions?: Record<string, { description?: string }> };
  const perms = doc.permissions ?? {};
  const conn = await createConnection(config.db);
  try {
    let count = 0;
    for (const [code, meta] of Object.entries(perms)) {
      const parts = code.split('_');
      const resource = parts.length >= 2 ? parts.slice(0, -1).join('_').toLowerCase() : null;
      const action = parts.length >= 2 ? parts[parts.length - 1].toLowerCase() : null;
      await conn.query(
        `INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'api', ?, ?, 0, 'enabled', NOW(), NOW())`,
        [nid(), code, meta?.description ?? code, meta?.description ?? null, resource, action],
      );
      count++;
    }
    return count;
  } finally {
    await conn.end();
  }
}

export default async function () {
  await preflight();
  await resetBase();
  const seeded = await seedPermissions();
  console.log(`[ui globalSetup] iam_permissions seeded: ${seeded}`);

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
