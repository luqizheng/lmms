import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createConnection } from 'mysql2/promise';
import { config } from '../config';

const __dirname = dirname(fileURLToPath(import.meta.url));

// 业务表：每套件前整表清空（据实际 schema）
const BUSINESS_TABLES = [
  'login_logs',
  'refresh_tokens',
  'refresh_markers',
  'revoked_jtis',
  'auth_codes',
  'user_roles',
  'user_org_units',
  'announcement_deliveries',
  'announcement_reads',
  'announcement_recipients',
  'announcements',
];

// 权限相关表：整轮测试前清空
const PERMISSION_TABLES = [
  'role_permissions',
  'permissions',
  'roles',
];

async function withConn<T>(
  fn: (c: Awaited<ReturnType<typeof createConnection>>) => Promise<T>,
): Promise<T> {
  const conn = await createConnection(config.db);
  try {
    return await fn(conn);
  } finally {
    await conn.end();
  }
}

// 整轮开始：清全部 + 重导基础 seed
export async function resetBase(): Promise<void> {
  await withConn(async (c) => {
    await c.query('SET FOREIGN_KEY_CHECKS = 0');
    for (const t of [...BUSINESS_TABLES, ...PERMISSION_TABLES, 'users']) {
      await c.query(`DELETE FROM ${t}`).catch(() => undefined);
    }
    const sql = readFileSync(join(__dirname, 'seed', 'base.sql'), 'utf8');
    for (const stmt of sql.split(';').map((s) => s.trim()).filter(Boolean)) {
      await c.query(stmt).catch(() => undefined);
    }
    await c.query('SET FOREIGN_KEY_CHECKS = 1');
  });
}

// 每套件前：清业务数据，保留基础数据
export async function resetBusiness(): Promise<void> {
  await withConn(async (c) => {
    await c.query('SET FOREIGN_KEY_CHECKS = 0');
    for (const t of BUSINESS_TABLES) {
      await c.query(`DELETE FROM ${t}`).catch(() => undefined);
    }
    // users 表保留基础数据：删除非种子 username 的行
    await c
      .query('DELETE FROM users WHERE username NOT IN (?)', [config.seedUsernames])
      .catch(() => undefined);
    await c.query('SET FOREIGN_KEY_CHECKS = 1');
  });
}
