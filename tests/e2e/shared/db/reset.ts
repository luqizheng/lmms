import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createConnection } from 'mysql2/promise';
import { config } from '../config';

const __dirname = dirname(fileURLToPath(import.meta.url));

// 业务表：每套件前整表清空（据实际 schema，Phase 4.0 表名前缀化）
const BUSINESS_TABLES = [
  'iam_login_logs',
  'audit_login_logs',
  'audit_operation_logs',
  'audit_api_access_logs',
  'iam_refresh_tokens',
  'token_refresh_markers',
  'token_revoked_jtis',
  'token_auth_codes',
  'iam_user_roles',
  'iam_user_org_units',
  'ntf_announcement_deliveries',
  'ntf_announcement_reads',
  'ntf_announcement_recipients',
  'ntf_announcements',
];

// 权限相关表：整轮测试前清空
const PERMISSION_TABLES = [
  'iam_role_permissions',
  'iam_permissions',
  'iam_roles',
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
    for (const t of [...BUSINESS_TABLES, ...PERMISSION_TABLES, 'iam_users']) {
      await c.query(`DELETE FROM ${t}`).catch(() => undefined);
    }
    // 保留 default tenant（Phase 8.1）— 只删自定义 tenant
    await c.query(`DELETE FROM iam_tenants WHERE id > 1`).catch(() => undefined);
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
    // iam_users 表保留基础数据：删除非种子 username 的行
    await c
      .query('DELETE FROM iam_users WHERE username NOT IN (?)', [config.seedUsernames])
      .catch(() => undefined);
    await c.query('SET FOREIGN_KEY_CHECKS = 1');
  });
}
