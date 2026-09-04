#!/usr/bin/env node
/**
 * scripts/demo-data.js
 *
 * 集成测试用 demo 数据生成器：向 infrastructure/docker-compose.yml 启动的
 * MySQL 容器插入全量业务数据（用户走 API、其余走 SQL）。
 *
 * 用法：
 *   node scripts/demo-data.js              # 追加模式（幂等 upsert）
 *   node scripts/demo-data.js --reset      # 先清空业务数据再插入（保留 default tenant）
 *   node scripts/demo-data.js --help
 *
 * pnpm 调用：
 *   pnpm docker:demo:data                  # 追加模式
 *   pnpm docker:demo:data -- --reset       # pnpm 转发参数到脚本
 *
 * 数据连接：默认从 infrastructure/.env 读 MYSQL_* 变量；环境变量可覆盖。
 * API 连接：从 E2E_BASE_URL（默认 http://localhost:3000 = api-gateway）走。
 */

const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const yaml = require('js-yaml');
const mysql = require('mysql2/promise');

const ROOT = path.join(__dirname, '..');
const ENV_FILE = path.join(ROOT, 'infrastructure', '.env');

// ---------- CLI ----------
const args = process.argv.slice(2);
const RESET = args.includes('--reset');
const HELP = args.includes('--help') || args.includes('-h');

if (HELP) {
  console.log(`usage: node scripts/demo-data.js [--reset]

  --reset   truncate business tables + users before inserting
            (default tenant + seed roles preserved via re-seed)

Reads connection info from infrastructure/.env (MYSQL_HOST/PORT/USER/PASSWORD/DATABASE)
and infrastructure/.env (E2E_BASE_URL for api-gateway).`);
  process.exit(0);
}

// ---------- 读 .env ----------
function loadEnv() {
  if (!fs.existsSync(ENV_FILE)) {
    throw new Error(`infrastructure/.env not found at ${ENV_FILE}`);
  }
  const out = {};
  for (const line of fs.readFileSync(ENV_FILE, 'utf8').split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const eq = trimmed.indexOf('=');
    if (eq < 0) continue;
    const key = trimmed.slice(0, eq).trim();
    let val = trimmed.slice(eq + 1).trim();
    // strip surrounding quotes
    if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
      val = val.slice(1, -1);
    }
    // expand ${VAR:-default} and ${VAR}
    val = val.replace(/\$\{([A-Z0-9_]+)(?::-([^}]*))?\}/g, (_, name, def) => {
      const v = process.env[name] ?? out[name];
      return v ?? (def ?? '');
    });
    out[key] = val;
  }
  return out;
}

const fileEnv = loadEnv();
const env = (k, fallback) => process.env[k] ?? fileEnv[k] ?? fallback;

// infrastructure/.env 里的 MYSQL_HOST 是 docker 服务名（"mysql"），
// 仅在 docker 网络内可解析。本脚本默认跑在 host，需要把 docker 服务名
// 翻成 localhost。环境变量显式传入则原样保留。
const DOCKER_SERVICE_HOSTS = new Set(['mysql', 'redis', 'consul', 'rabbitmq']);
function resolveMysqlHost() {
  const raw = env('MYSQL_HOST', 'localhost');
  if (process.env.MYSQL_HOST) return raw;
  if (DOCKER_SERVICE_HOSTS.has(raw)) return 'localhost';
  return raw;
}

const DB = {
  host: resolveMysqlHost(),
  port: parseInt(env('MYSQL_PORT', '3306'), 10),
  user: env('MYSQL_USER', env('MYSQL_ROOT_USER', 'root')),
  password: env('MYSQL_PASSWORD', env('MYSQL_ROOT_PASSWORD', 'rootpass')),
  database: env('MYSQL_DATABASE', 'superpowers'),
};
const API_BASE = env('E2E_BASE_URL', 'http://localhost:3000');
const MYSQL_CONTAINER = env('DEMO_MYSQL_CONTAINER', 'infrastructure-mysql-1');

// ---------- helpers ----------
const log = (...a) => console.log('[demo-data]', ...a);
const logSection = (s) => log('\n=== ' + s + ' ===');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 21-char nanoid-like id, url-safe, no padding (matches iam-service nanoid!())
const nid = () => {
  const alpha = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-';
  let out = '';
  for (let i = 0; i < 21; i++) out += alpha[Math.floor(Math.random() * alpha.length)];
  return out;
};
const now = () => new Date().toISOString().slice(0, 19).replace('T', ' ');
const daysAgo = (n) => new Date(Date.now() - n * 86400_000).toISOString().slice(0, 19).replace('T', ' ');

async function fetchJson(url, init = {}) {
  const res = await fetch(url, init);
  let body;
  try { body = await res.json(); } catch { body = null; }
  return { status: res.status, ok: res.ok, body };
}

async function fetchJsonAuthRetry(url, init, { retries = 15, intervalMs = 1500 } = {}) {
  for (let i = 0; i < retries; i++) {
    const r = await fetchJson(url, init);
    if (r.status !== 401) return r;
    if (i < retries - 1) await sleep(intervalMs);
  }
  return r;
}

// ---------- docker exec 兜底连接器 ----------
// 复用 mysql2/promise 的 query() 协议：query(sql, params) → [rows, fields]。
// 用于「MySQL 端口未发布到 host」时的 fallback：直接通过 docker exec 在容器内执行。
function escapeSqlValue(v) {
  if (v === null || v === undefined) return 'NULL';
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return v ? '1' : '0';
  if (v instanceof Date) return `'${v.toISOString().slice(0, 19).replace('T', ' ')}'`;
  if (typeof v === 'object') return `'${JSON.stringify(v).replace(/'/g, "\\'")}'`;
  return `'${String(v).replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
}
function renderSql(sql, params) {
  if (!params || params.length === 0) return sql;
  let i = 0;
  return sql.replace(/\?/g, () => escapeSqlValue(params[i++]));
}

function extractSelectColumns(sql) {
  const m = sql.match(/^\s*SELECT\s+([\s\S]+?)\s+FROM\s+/i);
  if (!m) return null;
  const colsPart = m[1].trim();
  const cols = [];
  let depth = 0;
  let cur = '';
  for (const ch of colsPart) {
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    else if (ch === ',' && depth === 0) {
      cols.push(cur.trim());
      cur = '';
      continue;
    }
    cur += ch;
  }
  if (cur.trim()) cols.push(cur.trim());
  return cols.map((c) => {
    if (c === '*') return 'col_0';
    const last = c.split(/\s+/).pop().replace(/`/g, '');
    return last;
  });
}

function execMysql(container, db, sql, params) {
  return new Promise((resolve, reject) => {
    const rendered = renderSql(sql, params);
    const args = [
      'exec', '-i', container,
      'mysql',
      '-u' + db.user,
      '-p' + db.password,
      '-h', '127.0.0.1',
      '--default-character-set=utf8mb4',
      '-B', '-N',
      db.database,
      '-e', rendered,
    ];
    const proc = spawn('docker', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', (d) => stdout += d.toString());
    proc.stderr.on('data', (d) => stderr += d.toString());
    proc.on('error', reject);
    proc.on('close', (code) => {
      if (code !== 0) return reject(new Error(`docker exec mysql failed (exit ${code}): ${stderr.trim()}`));
      const lines = stdout.split('\n').filter((l) => l.length > 0);
      if (lines.length === 0) return resolve([[], []]);
      const colNames = extractSelectColumns(rendered) || [];
      const fields = [];
      const rows = lines.map((l, i) => {
        const arr = l.split('\t');
        const obj = {};
        for (let k = 0; k < arr.length; k++) {
          const name = colNames[k] ?? `col_${k}`;
          if (i === 0) fields.push({ name });
          obj[name] = arr[k];
        }
        return obj;
      });
      resolve([rows, fields]);
    });
  });
}

class DockerExecConn {
  constructor(container, db) { this.container = container; this.db = db; }
  async query(sql, params) { return execMysql(this.container, this.db, sql, params); }
  async end() { return undefined; }
}

async function openDbConnection() {
  try {
    const conn = await mysql.createConnection({ ...DB, multipleStatements: false });
    return { conn, mode: 'direct' };
  } catch (e) {
    if (e.code !== 'ECONNREFUSED' && e.code !== 'ETIMEDOUT' && e.code !== 'ENOTFOUND') throw e;
    log(`direct connect failed (${e.code}); falling back to docker exec on ${MYSQL_CONTAINER}`);
    return { conn: new DockerExecConn(MYSQL_CONTAINER, DB), mode: 'docker-exec' };
  }
}

// ---------- 主流程 ----------
async function main() {
  log('connecting mysql', { host: DB.host, port: DB.port, db: DB.database, user: DB.user });
  const { conn, mode } = await openDbConnection();
  log(`connected via ${mode}`);

  try {
    if (RESET) {
      logSection('--reset mode: truncate business tables');
      await resetBusinessTables(conn);
    }

    logSection('health check: api-gateway + iam-service');
    await preflight();

    logSection('register users via API');
    await seedIamPermissions(conn);
    const users = await registerUsers(conn);

    logSection('login as admin to get token');
    const adminToken = await loginAdmin();
    log('admin token acquired');

    logSection('iam: tenants / org-units / roles / permissions / menus / mappings');
    await seedIam(conn, adminToken, users);

    logSection('notification: announcements / templates');
    await seedNotification(conn, adminToken, users);

    logSection('audit: login / operation / api-access logs');
    await seedAudit(conn, users);

    logSection('file: file metadata + presigned URLs');
    await seedFile(conn, users);

    logSection('config: namespaces + items + versions');
    await seedConfig(conn);

    logSection('token: refresh markers + revoked JTIs (sample)');
    await seedToken(conn, users);

    logSection('summary');
    await summarize(conn);
    log('done. demo data ready for integration testing.');
  } finally {
    await conn.end();
  }
}

// ---------- 业务表清空 ----------
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
  'ntf_templates',
  'ntf_user_projection',
  'iam_role_permissions',
  'iam_user_roles',
  'iam_users',
  'iam_org_units',
  'iam_menus',
  'audit_events',
  'file_presign_urls',
  'file_files',
  'cfg_config_items',
  'cfg_config_versions',
  'cfg_namespaces',
  'token_oauth_clients',
  'prue_menu_configs',
];

async function resetBusinessTables(conn) {
  // docker-exec 模式每条 query 是新连接，FK_CHECK 设置跨调用不持久；
  // 所以把全部 DELETE 合并到一条 SQL，用 ; 分隔。
  const stmts = ['SET FOREIGN_KEY_CHECKS = 0'];
  for (const t of BUSINESS_TABLES) {
    stmts.push(`DELETE FROM ${t}`);
  }
  stmts.push('DELETE FROM iam_permissions');
  stmts.push('DELETE FROM iam_roles');
  stmts.push('DELETE FROM iam_tenants WHERE id > 1');
  stmts.push('SET FOREIGN_KEY_CHECKS = 1');
  const batch = stmts.join('; ');
  try {
    await conn.query(batch);
  } catch (e) {
    // 某些表可能不存在（schema 演进），逐条重试跳过缺失表
    for (const s of stmts) {
      try { await conn.query(s); } catch (inner) {
        if (!String(inner.message).includes("doesn't exist")) throw inner;
      }
    }
  }
  log('cleared business tables');
}

// ---------- health ----------
async function preflight() {
  for (const ep of [`${API_BASE}/v1/iam/health`, `${API_BASE}/health`]) {
    const r = await fetch(ep).catch(() => null);
    if (r && r.ok) { log('api-gateway reachable via', ep); return; }
  }
  throw new Error(`api-gateway unreachable at ${API_BASE}. Start with: pnpm run dev:infra && pnpm run dev`);
}

// ---------- 用户 ----------
const USERS = [
  // username, nickname, email, role_code (空=不绑定具体角色)
  { username: 'admin',  nickname: '系统管理员',  email: 'admin@demo.local',  password: 'Demo@123', role: 'platform_admin' },
  { username: 'alice',  nickname: '爱丽丝',       email: 'alice@demo.local',  password: 'Demo@123', role: 'tenant_admin' },
  { username: 'bob',    nickname: '鲍勃-经理',    email: 'bob@demo.local',    password: 'Demo@123', role: 'manager' },
  { username: 'carol',  nickname: '卡罗尔',       email: 'carol@demo.local',  password: 'Demo@123', role: 'user' },
  { username: 'david',  nickname: '大卫',         email: 'david@demo.local',  password: 'Demo@123', role: 'user' },
  { username: 'eve',    nickname: '伊芙',         email: 'eve@demo.local',    password: 'Demo@123', role: 'user' },
  { username: 'frank',  nickname: '弗兰克-销售',  email: 'frank@demo.local',  password: 'Demo@123', role: 'manager' },
  { username: 'grace',  nickname: '格蕾丝-审计',  email: 'grace@demo.local',  password: 'Demo@123', role: 'auditor' },
  { username: 'henry',  nickname: '亨利-配置',    email: 'henry@demo.local',  password: 'Demo@123', role: 'config_admin' },
  { username: 'ivy',    nickname: '艾薇',         email: 'ivy@demo.local',    password: 'Demo@123', role: 'user' },
];

async function registerUsers(conn) {
  // iam /iam/auth/register 公开接口只允许注册首用户（自动变 tenant_admin）；
  // 后续用户必须 admin 鉴权后通过 POST /iam/users 创建。
  // query() 按 mysql2 约定返回 [rows, fields]，需双层解构；docker-exec 回退
  // 通道下 COUNT 值为字符串，Number() 归一化否则 '0' === 0 为 false。
  const [countRows] = await conn.query(
    "SELECT COUNT(*) c FROM iam_users WHERE deleted_at IS NULL"
  );
  const userCount = Number(countRows[0].c);

  const mustClearSeedRoles = userCount === 0;
  if (mustClearSeedRoles) {
    log('detected fresh DB, clearing seed roles to allow first-user registration');
    await conn.query("DELETE FROM iam_role_permissions WHERE role_id IN ('role-tenant-admin-seed', 'role-platform-admin-seed')");
    await conn.query("DELETE FROM iam_user_roles WHERE role_id IN ('role-tenant-admin-seed', 'role-platform-admin-seed')");
    await conn.query("DELETE FROM iam_roles WHERE id IN ('role-tenant-admin-seed', 'role-platform-admin-seed')");
  }

  const first = USERS[0];

  // Gate: 若 DB 已有其他用户但 admin (USERS[0]) 缺失，追加模式无路可走 —
  // register 端点会返回 403 (users exist)，随后 loginAs admin 必然 401。
  // 提前抛错要求 --reset，避免误导用户以为是服务挂了。
  if (userCount > 0) {
    const [adminRows] = await conn.query(
      "SELECT id FROM iam_users WHERE username = ? AND deleted_at IS NULL",
      [first.username]
    );
    if (adminRows.length === 0) {
      throw new Error(
        `iam_users has ${userCount} row(s) but seed admin "${first.username}" is missing. ` +
        `Public /iam/auth/register is closed once any user exists, so demo-data cannot bootstrap admin in append mode. ` +
        `Run: node scripts/demo-data.js --reset`
      );
    }
  }

  const reg0 = await fetchJson(`${API_BASE}/v1/iam/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: first.username, password: first.password, nickname: first.nickname, email: first.email }),
  });
  if (reg0.status === 403) {
    log(`  · registration closed (users exist), skip ${first.username}`);
  } else if (reg0.status === 409 || (reg0.body && reg0.body.code === 'IAM_AUTH_USER_EXISTS')) {
    log(`  · ${first.username} exists, skip`);
  } else if (reg0.ok || reg0.status === 201) {
    log(`  ✓ registered ${first.username} (via public register → auto tenant_admin)`);
  } else {
    throw new Error(`register ${first.username} failed: ${reg0.status} ${JSON.stringify(reg0.body)}`);
  }

  const adminToken = await loginAs(first.username, first.password);

  for (const u of USERS.slice(1)) {
    const [existsRows] = await conn.query(
      "SELECT id FROM iam_users WHERE username = ? AND deleted_at IS NULL",
      [u.username]
    );
    if (existsRows.length > 0) {
      log(`  · ${u.username} exists, skip`);
      continue;
    }
    const res = await fetchJson(`${API_BASE}/v1/iam/users`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${adminToken}` },
      body: JSON.stringify({ username: u.username, password: u.password, nickname: u.nickname, email: u.email }),
    });
    if (res.ok || res.status === 201) {
      log(`  ✓ created ${u.username}`);
    } else if (res.status === 409 || (res.body && res.body.code === 'IAM_USER_EXISTS')) {
      log(`  · ${u.username} exists (409)`);
    } else {
      log(`  ✗ create ${u.username} failed: ${res.status}`, res.body);
      throw new Error(`create ${u.username} failed: ${res.status}`);
    }
  }

  if (mustClearSeedRoles) {
    log('re-seeding platform_admin role');
    await conn.query(
      `INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at)
       VALUES ('role-platform-admin-seed', 1, '平台管理员', 'platform_admin', '平台管理员（demo seed）', 'all', 'active', NOW(), NOW())
       ON DUPLICATE KEY UPDATE id = id`
    );
    await conn.query(
      `INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
       SELECT 'role-platform-admin-seed', id, NOW() FROM iam_permissions
       WHERE status = 'enabled' AND deleted_at IS NULL`
    );
  }

  // 收集 id+tenant_id
  const placeholders = USERS.map(() => '?').join(',');
  const [rows] = await conn.query(
    `SELECT id, username, tenant_id FROM iam_users WHERE deleted_at IS NULL AND username IN (${placeholders})`,
    USERS.map((u) => u.username)
  );
  const byName = new Map(rows.map((r) => [r.username, r]));
  return USERS.map((u) => ({ ...u, id: byName.get(u.username)?.id, tenant_id: byName.get(u.username)?.tenant_id }));
}

async function loginAs(username, password) {
  const r = await fetchJson(`${API_BASE}/v1/iam/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  if (!r.ok || !r.body?.access_token) {
    throw new Error(`login ${username} failed: ${r.status} ${JSON.stringify(r.body)}`);
  }
  return r.body.access_token;
}

async function loginAdmin() {
  return loginAs('admin', 'Demo@123');
}

// ---------- IAM 域 ----------
async function seedIamPermissions(conn) {
  const yamlPath = path.join(ROOT, 'contracts', 'permissions.yaml');
  const doc = yaml.load(fs.readFileSync(yamlPath, 'utf8'));
  const perms = doc.permissions || {};
  let count = 0;
  for (const [code, meta] of Object.entries(perms)) {
    const id = nid();
    const name = (meta && meta.description) || code;
    const desc = meta && meta.description;
    const parts = code.split('_');
    const resource = parts.length >= 2 ? parts.slice(0, -1).join('_').toLowerCase() : null;
    const action = parts.length >= 2 ? parts[parts.length - 1].toLowerCase() : null;
    await conn.query(
      `INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
       VALUES (?, ?, ?, ?, 'api', ?, ?, 0, 'enabled', NOW(), NOW())
       ON DUPLICATE KEY UPDATE name = VALUES(name), description = VALUES(description)`,
      [id, code, name, desc, resource, action]
    );
    count++;
  }
  log(`  · iam_permissions seeded (${count})`);
}
async function seedIam(conn, adminToken, users) {
  // tenants (id=1 已存在，id=2 演示用)
  await conn.query(
    `INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at)
     VALUES (2, 'acme-corp', 'ACME 公司', 'active', NOW(), NOW())
     ON DUPLICATE KEY UPDATE slug = VALUES(slug)`
  );
  log('  · tenants seeded');

  // 先用 code 查已存在的 org id，避免重新生成 nanoid 后破坏 parent_id FK
  const orgCodes = ['HQ', 'ENG', 'PROD', 'SALES'];
  const orgIds = {};
  const [existingOrgs] = await conn.query(
    `SELECT id, code FROM iam_org_units WHERE tenant_id = 1 AND code IN (?, ?, ?, ?)`,
    orgCodes
  );
  for (const r of existingOrgs) orgIds[r.code] = r.id;
  for (const code of orgCodes) {
    if (!orgIds[code]) {
      const newId = nid();
      orgIds[code] = newId;
      const parentId = code === 'HQ' ? null : orgIds.HQ;
      const level = code === 'HQ' ? 1 : 2;
      const sort = orgCodes.indexOf(code);
      const names = { HQ: '总部', ENG: '研发部', PROD: '产品部', SALES: '销售部' };
      await conn.query(
        `INSERT INTO iam_org_units (id, parent_id, name, code, tenant_id, level, sort_order, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, 1, ?, ?, 'active', NOW(), NOW())
         ON DUPLICATE KEY UPDATE name = VALUES(name), sort_order = VALUES(sort_order)`,
        [newId, parentId, names[code], code, level, sort]
      );
    }
  }
  const orgHQ = orgIds.HQ;
  log('  · org_units seeded (HQ/ENG/PROD/SALES)');

  // 把 admin/alice/grace/henry → HQ primary; bob/carol/david/eve → ENG; frank → SALES; ivy → PROD
  const orgMap = {
    admin: orgHQ, alice: orgHQ, grace: orgHQ, henry: orgHQ,
    bob: orgIds.ENG, carol: orgIds.ENG, david: orgIds.ENG, eve: orgIds.ENG,
    frank: orgIds.SALES, ivy: orgIds.PROD,
  };
  for (const u of users) {
    const orgId = orgMap[u.username];
    if (!orgId) continue;
    await conn.query(
      `INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at)
       VALUES (?, ?, 1, NOW())
       ON DUPLICATE KEY UPDATE is_primary = 1`,
      [u.id, orgId]
    );
  }
  log('  · user_org_units assigned');

  // roles：自定义 4 个 (manager/user/auditor/config_admin)；tenant_admin + platform_admin 已存在
  const roleIds = {};
  const customRoles = [
    { code: 'manager',      name: '部门经理',   data_scope: 'dept',         description: '部门经理' },
    { code: 'user',         name: '普通用户',   data_scope: 'self',         description: '普通用户' },
    { code: 'auditor',      name: '审计员',     data_scope: 'all',          description: '审计员' },
    { code: 'config_admin', name: '配置管理员', data_scope: 'all',          description: '配置管理员' },
  ];
  for (const r of customRoles) {
    const id = nid();
    await conn.query(
      `INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at)
       VALUES (?, 1, ?, ?, ?, ?, 'active', NOW(), NOW())
       ON DUPLICATE KEY UPDATE name = VALUES(name)`,
      [id, r.name, r.code, r.description, r.data_scope]
    );
    roleIds[r.code] = id;
  }
  // 读出 seed 角色的 id
  const [seedRows] = await conn.query(
    "SELECT id, code FROM iam_roles WHERE code IN ('tenant_admin','platform_admin') AND deleted_at IS NULL"
  );
  for (const r of seedRows) roleIds[r.code] = r.id;
  log('  · roles seeded (manager/user/auditor/config_admin + seed tenant/platform)');

  // role_permissions：来自 contracts/permissions.yaml 的子集
  const PERM_MATRIX = {
    platform_admin: null, // null = all
    tenant_admin: [
      'IAM_USER_CREATE', 'IAM_USER_READ', 'IAM_USER_UPDATE', 'IAM_USER_DELETE',
      'IAM_ROLE_CREATE', 'IAM_ROLE_READ', 'IAM_ROLE_UPDATE', 'IAM_ROLE_DELETE',
      'IAM_PERM_READ', 'IAM_PERM_ASSIGN',
      'IAM_MENU_CREATE', 'IAM_MENU_READ', 'IAM_MENU_UPDATE', 'IAM_MENU_DELETE',
      'IAM_ORG_CREATE', 'IAM_ORG_READ', 'IAM_ORG_UPDATE', 'IAM_ORG_DELETE',
      'NTF_TEMPLATE_CREATE', 'NTF_TEMPLATE_READ', 'NTF_TEMPLATE_UPDATE', 'NTF_TEMPLATE_DELETE',
      'NTF_MESSAGE_READ',
      'FILE_UPLOAD', 'FILE_DOWNLOAD', 'FILE_DELETE', 'FILE_READ',
      'CFG_NAMESPACE_CREATE', 'CFG_NAMESPACE_READ', 'CFG_NAMESPACE_UPDATE', 'CFG_NAMESPACE_DELETE',
      'CFG_CONFIG_CREATE', 'CFG_CONFIG_READ', 'CFG_CONFIG_UPDATE', 'CFG_CONFIG_DELETE',
    ],
    manager: ['IAM_USER_READ', 'IAM_ORG_READ', 'IAM_MENU_READ', 'NTF_MESSAGE_READ', 'FILE_READ'],
    user: ['IAM_MENU_READ', 'NTF_MESSAGE_READ'],
    auditor: ['AUD_LOG_READ', 'AUD_LOG_EXPORT', 'IAM_USER_READ', 'IAM_ROLE_READ'],
    config_admin: [
      'CFG_CONFIG_CREATE', 'CFG_CONFIG_READ', 'CFG_CONFIG_UPDATE', 'CFG_CONFIG_DELETE', 'CFG_CONFIG_REVEAL',
      'CFG_NAMESPACE_CREATE', 'CFG_NAMESPACE_READ', 'CFG_NAMESPACE_UPDATE', 'CFG_NAMESPACE_DELETE',
      'CFG_NAMESPACE_EXPORT', 'CFG_NAMESPACE_IMPORT',
      'IAM_MENU_READ',
    ],
  };
  const [allPerms] = await conn.query("SELECT id, code FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL");
  const permByCode = new Map(allPerms.map((p) => [p.code, p.id]));
  for (const [roleCode, codes] of Object.entries(PERM_MATRIX)) {
    const rid = roleIds[roleCode];
    if (!rid) continue;
    if (codes === null) {
      // all perms
      for (const pid of permByCode.values()) {
        await conn.query(
          `INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at) VALUES (?, ?, NOW())`,
          [rid, pid]
        );
      }
    } else {
      for (const code of codes) {
        const pid = permByCode.get(code);
        if (!pid) { log(`    ! perm not found: ${code} (skip)`); continue; }
        await conn.query(
          `INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at) VALUES (?, ?, NOW())`,
          [rid, pid]
        );
      }
    }
  }
  log('  · role_permissions assigned');

  // user_roles
  for (const u of users) {
    const rid = roleIds[u.role];
    if (!rid) continue;
    await conn.query(
      `INSERT IGNORE INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())`,
      [u.id, rid]
    );
  }
  log('  · user_roles assigned');

  // menus
  const menus = [
    { name: '工作台',      code: 'dashboard',       path: '/dashboard',           component: 'Dashboard',       sort: 1 },
    { name: '用户管理',    code: 'iam-users',       path: '/iam/users',           component: 'IamUsers',        sort: 10 },
    { name: '角色管理',    code: 'iam-roles',       path: '/iam/roles',           component: 'IamRoles',        sort: 11 },
    { name: '部门管理',    code: 'iam-orgs',        path: '/iam/org-units',       component: 'IamOrgs',         sort: 12 },
    { name: '审计日志',    code: 'audit-logs',      path: '/audit/login',         component: 'AuditLogs',       sort: 20 },
    { name: '通知公告',    code: 'notification',    path: '/notification/list',   component: 'Notification',    sort: 30 },
    { name: '文件管理',    code: 'files',           path: '/file/list',           component: 'Files',           sort: 40 },
    { name: '配置中心',    code: 'config',          path: '/config/item',         component: 'Config',          sort: 50 },
  ];
  const menuIds = {};
  for (const m of menus) {
    const id = nid();
    await conn.query(
      `INSERT INTO iam_menus (id, parent_id, name, code, path, component, icon, sort_order, visible, status, created_at, updated_at)
       VALUES (?, NULL, ?, ?, ?, ?, NULL, ?, 1, 'enabled', NOW(), NOW())
       ON DUPLICATE KEY UPDATE name = VALUES(name), sort_order = VALUES(sort_order)`,
      [id, m.name, m.code, m.path, m.component, m.sort]
    );
    menuIds[m.code] = id;
  }
  log('  · menus seeded');

  // 给 menus 绑 permission_id（可选：菜单点击需要的权限）
  const menuPermMap = {
    'iam-users': 'IAM_USER_READ',
    'iam-roles': 'IAM_ROLE_READ',
    'iam-orgs': 'IAM_ORG_READ',
    'audit-logs': 'AUD_LOG_READ',
    'files': 'FILE_READ',
    'config': 'CFG_CONFIG_READ',
  };
  for (const [code, permCode] of Object.entries(menuPermMap)) {
    const mid = menuIds[code];
    const pid = permByCode.get(permCode);
    if (mid && pid) {
      await conn.query(`UPDATE iam_menus SET permission_id = ? WHERE id = ?`, [pid, mid]);
    }
  }
  log('  · menu permissions bound');
}

// ---------- Notification 域 ----------
async function seedNotification(conn, adminToken, users) {
  // templates
  const templates = [
    { code: 'login_welcome',   title: '登录欢迎',         body: '欢迎 {{nickname}} 登录系统！' },
    { code: 'password_reset',  title: '密码重置',         body: '您的密码已重置，新密码请妥善保管。' },
    { code: 'role_changed',    title: '角色变更通知',     body: '您的角色已变更，请重新登录。' },
  ];
  for (const t of templates) {
    await conn.query(
      `INSERT INTO ntf_templates (code, title, body, updated_at)
       VALUES (?, ?, ?, NOW())
       ON DUPLICATE KEY UPDATE title = VALUES(title), body = VALUES(body)`,
      [t.code, t.title, t.body]
    );
  }
  log('  · ntf_templates seeded');

  // announcements (3 条)
  const announcements = [
    { title: '欢迎使用 coder-micro-services', body: '本系统为多租户微服务平台，支持用户/角色/权限/审计/通知/文件/配置全流程管理。', pinned: 1, popover: 1, scope: 'all' },
    { title: '新功能发布：审计日志增强',         body: '审计模块新增导出 CSV、按时间段过滤能力，欢迎试用。', pinned: 0, popover: 0, scope: 'role:user' },
    { title: '系统维护通知',                     body: '本周六凌晨 02:00 - 04:00 进行例行维护，期间服务可能短暂不可用。', pinned: 0, popover: 0, scope: 'all' },
  ];
  for (const a of announcements) {
    const publishAt = daysAgo(Math.floor(Math.random() * 7));
    const [r] = await conn.query(
      `INSERT INTO ntf_announcements (tenant_id, title, body, publish_at, pinned, popover, status, created_by, created_at)
       VALUES (1, ?, ?, ?, ?, ?, 'published', 'admin', NOW())`,
      [a.title, a.body, publishAt, a.pinned, a.popover]
    );
    const aid = r.insertId;
    if (a.scope === 'all') {
      // scope_type=user + scope_id 为特殊 'all'，此处用 role 维度给所有人
      await conn.query(
        `INSERT IGNORE INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id)
         SELECT ?, 'user', id FROM iam_users WHERE deleted_at IS NULL`,
        [aid]
      );
    } else if (a.scope.startsWith('role:')) {
      const code = a.scope.slice(5);
      const [{ rid }] = await conn.query("SELECT id rid FROM iam_roles WHERE code = ? AND deleted_at IS NULL", [code]);
      if (rid) {
        await conn.query(
          `INSERT IGNORE INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?, 'role', ?)`,
          [aid, rid]
        );
      }
    }
  }
  log('  · ntf_announcements seeded (3)');

  // user_projection：每个用户的角色快照
  for (const u of users) {
    if (!u.id) continue;
    const [roles] = await conn.query(
      `SELECT r.code FROM iam_user_roles ur JOIN iam_roles r ON r.id = ur.role_id WHERE ur.user_id = ?`,
      [u.id]
    );
    const roleIds = roles.map((r) => r.code);
    const primaryOrg = orgMapOf(u.username);
    await conn.query(
      `INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at)
       VALUES (?, ?, CAST(? AS JSON), ?, ?)
       ON DUPLICATE KEY UPDATE role_ids = VALUES(role_ids), dept_id = VALUES(dept_id), last_event_at = VALUES(last_event_at)`,
      [u.id, u.tenant_id || 1, JSON.stringify(roleIds), primaryOrg, Date.now()]
    );
  }
  log('  · ntf_user_projection seeded');
}

// 与 iam 域共享的 org 映射（demo 用，固定 ID）
function orgMapOf(username) {
  const map = {
    admin: 'HQ', alice: 'HQ', grace: 'HQ', henry: 'HQ',
    bob: 'ENG', carol: 'ENG', david: 'ENG', eve: 'ENG',
    frank: 'SALES', ivy: 'PROD',
  };
  return map[username] || 'HQ';
}

// ---------- Audit 域 ----------
async function seedAudit(conn, users) {
  const usernames = users.map((u) => u.username);
  const locations = ['北京', '上海', '深圳', '杭州', '成都', '远程'];
  const userAgents = [
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36',
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/126.0',
    'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0) Mobile/15E148 Safari/604.1',
    'curl/8.4.0',
    'PostmanRuntime/7.39.0',
  ];
  const failReasons = ['InvalidCredentials', 'AccountLocked', 'PasswordExpired', 'MfaRequired'];

  // login_logs：20 条（15 成功 + 5 失败）
  for (let i = 0; i < 20; i++) {
    const u = users[Math.floor(Math.random() * users.length)];
    const success = i < 15 ? 1 : 0;
    const failReason = success ? null : failReasons[Math.floor(Math.random() * failReasons.length)];
    await conn.query(
      `INSERT INTO audit_login_logs (user_id, username, success, fail_reason, ip_address, location, user_agent, request_id, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      [
        success ? u.id : null, u.username, success, failReason,
        `192.168.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
        locations[Math.floor(Math.random() * locations.length)],
        userAgents[Math.floor(Math.random() * userAgents.length)],
        nid(),
        daysAgo(Math.floor(Math.random() * 30)),
      ]
    );
  }
  // 同时灌 iam_login_logs（两份 schema 略有差异）
  for (let i = 0; i < 20; i++) {
    const u = users[Math.floor(Math.random() * users.length)];
    const success = i < 15 ? 1 : 0;
    await conn.query(
      `INSERT INTO iam_login_logs (id, user_id, tenant_id, username, ip_address, user_agent, location, login_type, success, error_message, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, 'password', ?, ?, ?)`,
      [
        nid(),
        success ? u.id : null, u.tenant_id || 1, u.username,
        `10.0.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
        userAgents[Math.floor(Math.random() * userAgents.length)],
        locations[Math.floor(Math.random() * locations.length)],
        success,
        success ? null : 'invalid credentials',
        daysAgo(Math.floor(Math.random() * 30)),
      ]
    );
  }
  log('  · audit_login_logs + iam_login_logs seeded (20 + 20)');

  // operation_logs：30 条
  const actions = ['CREATE', 'UPDATE', 'DELETE', 'READ', 'EXPORT'];
  const resources = ['iam.user', 'iam.role', 'iam.permission', 'iam.org_unit', 'cfg.config_item', 'file.file', 'ntf.announcement'];
  for (let i = 0; i < 30; i++) {
    const u = users[Math.floor(Math.random() * users.length)];
    const action = actions[Math.floor(Math.random() * actions.length)];
    const resource = resources[Math.floor(Math.random() * resources.length)];
    await conn.query(
      `INSERT INTO audit_operation_logs (user_id, action, resource, resource_id, before_value, after_value, request_id, ip_address, user_agent, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      [
        u.id, action, resource, nid(),
        action === 'CREATE' ? null : JSON.stringify({ status: 'active' }),
        action === 'DELETE' ? null : JSON.stringify({ status: 'active' }),
        nid(),
        `172.16.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
        userAgents[Math.floor(Math.random() * userAgents.length)],
        daysAgo(Math.floor(Math.random() * 30)),
      ]
    );
  }
  log('  · audit_operation_logs seeded (30)');

  // api_access_logs：50 条
  const services = ['iam-service', 'notification-service', 'config-service', 'file-service', 'audit-service', 'token-service'];
  const methods = ['GET', 'POST', 'PUT', 'DELETE'];
  const paths = ['/iam/users', '/iam/roles', '/iam/permissions', '/iam/org-units',
                 '/notification/announcements', '/config/item', '/config/namespace',
                 '/file/list', '/audit/login', '/audit/operation', '/token/refresh'];
  for (let i = 0; i < 50; i++) {
    const svc = services[Math.floor(Math.random() * services.length)];
    const m = methods[Math.floor(Math.random() * methods.length)];
    const p = paths[Math.floor(Math.random() * paths.length)];
    const code = Math.random() < 0.95 ? (m === 'GET' ? 200 : 200) : [400, 401, 403, 404, 500][Math.floor(Math.random() * 5)];
    await conn.query(
      `INSERT INTO audit_api_access_logs (service_name, method, path, status_code, duration_ms, user_id, request_id, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      [
        svc, m, p, code,
        Math.floor(Math.random() * 500) + 5,
        users[Math.floor(Math.random() * users.length)].id,
        nid(),
        daysAgo(Math.floor(Math.random() * 30)),
      ]
    );
  }
  log('  · audit_api_access_logs seeded (50)');

  // audit_events：10 条
  const eventTypes = ['iam.user.created', 'iam.user.deleted', 'iam.role.assigned', 'ntf.announcement.published'];
  for (let i = 0; i < 10; i++) {
    await conn.query(
      `INSERT INTO audit_events (event_id, event_type, occurred_at, producer, payload_json)
       VALUES (?, ?, ?, 'iam-service', CAST(? AS JSON))`,
      [nid(), eventTypes[Math.floor(Math.random() * eventTypes.length)], daysAgo(Math.floor(Math.random() * 7)), JSON.stringify({ demo: true })]
    );
  }
  log('  · audit_events seeded (10)');
}

// ---------- File 域 ----------
async function seedFile(conn, users) {
  const files = [
    { filename: 'logo.png',         content_type: 'image/png',         size: 12345,    storage_path: '/storage/demo/logo.png' },
    { filename: 'README.md',        content_type: 'text/markdown',     size: 4321,     storage_path: '/storage/demo/README.md' },
    { filename: 'data-export.xlsx', content_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', size: 98765, storage_path: '/storage/demo/data-export.xlsx' },
    { filename: 'config-backup.yaml', content_type: 'application/yaml', size: 2345,     storage_path: '/storage/demo/config-backup.yaml' },
    { filename: 'audit-q3.csv',     content_type: 'text/csv',          size: 543210,   storage_path: '/storage/demo/audit-q3.csv' },
  ];
  for (let i = 0; i < files.length; i++) {
    const f = files[i];
    const id = nid();
    const owner = users[i % users.length];
    await conn.query(
      `INSERT INTO file_files (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, 1, ?, 0, NULL, ?, ?)`,
      [id, f.filename, f.content_type, f.size, nid().slice(0, 32), f.storage_path, owner.id, now(), now()]
    );
    // 配 1-2 个 presigned url
    const opCount = 1 + Math.floor(Math.random() * 2);
    for (let k = 0; k < opCount; k++) {
      await conn.query(
        `INSERT INTO file_presign_urls (id, file_id, operation, url, expires_at, created_by, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
        [nid(), id, ['GET', 'PUT'][Math.floor(Math.random() * 2)], `https://demo.local/presign/${id}?sig=${nid().slice(0, 16)}`, daysAgo(-1), owner.id, now()]
      );
    }
  }
  log('  · file_files + file_presign_urls seeded');
}

// ---------- Config 域 ----------
async function seedConfig(conn) {
  const ns = [
    { app: 'common',  env: 'prod', description: '公共配置' },
    { app: 'billing', env: 'prod', description: '计费配置' },
    { app: 'feature', env: 'prod', description: '功能开关' },
  ];
  const nsIds = {};
  for (const n of ns) {
    const id = nid();
    await conn.query(
      `INSERT INTO cfg_namespaces (id, tenant_id, app, env, profile, description, created_at, updated_at)
       VALUES (?, 1, ?, ?, 'default', ?, NOW(), NOW())
       ON DUPLICATE KEY UPDATE description = VALUES(description)`,
      [id, n.app, n.env, n.description]
    );
    const [rows] = await conn.query("SELECT id FROM cfg_namespaces WHERE tenant_id = 1 AND app = ? AND env = ? AND profile = 'default'", [n.app, n.env]);
    nsIds[n.app] = rows[0].id;
  }
  log('  · cfg_namespaces seeded');

  const items = [
    { ns: 'common',  key: 'site.name',          value: 'coder-micro-services', value_type: 'string', is_secret: 0 },
    { ns: 'common',  key: 'upload.max_size_mb', value: '100',                  value_type: 'number', is_secret: 0 },
    { ns: 'common',  key: 'support.email',      value: 'support@demo.local',   value_type: 'string', is_secret: 0 },
    { ns: 'billing', key: 'price.starter',      value: '99',                   value_type: 'number', is_secret: 0 },
    { ns: 'billing', key: 'price.pro',          value: '499',                  value_type: 'number', is_secret: 0 },
    { ns: 'billing', key: 'stripe.secret_key',  value: 'sk_test_demo_PLACEHOLDER', value_type: 'string', is_secret: 1 },
    { ns: 'feature', key: 'flag.beta_chat',     value: 'true',                 value_type: 'bool',   is_secret: 0 },
    { ns: 'feature', key: 'flag.dark_mode',     value: 'true',                 value_type: 'bool',   is_secret: 0 },
  ];
  for (const it of items) {
    const itemId = nid();
    await conn.query(
      `INSERT INTO cfg_config_items (id, namespace_id, \`key\`, value, value_type, is_secret, description, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, NULL, NOW(), NOW())
       ON DUPLICATE KEY UPDATE value = VALUES(value), value_type = VALUES(value_type), is_secret = VALUES(is_secret)`,
      [itemId, nsIds[it.ns], it.key, it.value, it.value_type, it.is_secret]
    );
    const [itemRows] = await conn.query(
      "SELECT id FROM cfg_config_items WHERE namespace_id = ? AND `key` = ?",
      [nsIds[it.ns], it.key]
    );
    const realItemId = itemRows[0].id;
    await conn.query(
      `INSERT INTO cfg_config_versions (id, namespace_id, item_id, \`key\`, old_value, new_value, operator, comment, created_at)
       VALUES (?, ?, ?, ?, '', ?, 'admin', 'demo seed', NOW())`,
      [nid(), nsIds[it.ns], realItemId, it.key, it.value]
    );
  }
  log('  · cfg_config_items + cfg_config_versions seeded');
}

// ---------- Token 域 ----------
async function seedToken(conn, users) {
  // refresh_markers：3 条
  for (let i = 0; i < 3; i++) {
    const jti = 'demo-jti-' + nid();
    await conn.query(
      `INSERT INTO token_refresh_markers (jti, expires_at) VALUES (?, ?)`,
      [jti, daysAgo(-7)]
    );
  }
  // revoked_jtis：3 条
  for (let i = 0; i < 3; i++) {
    const jti = 'demo-revoked-' + nid();
    await conn.query(
      `INSERT INTO token_revoked_jtis (jti, expires_at) VALUES (?, ?)`,
      [jti, daysAgo(-30)]
    );
  }
  // iam_refresh_tokens：5 条
  for (let i = 0; i < 5; i++) {
    const u = users[i % users.length];
    if (!u.id) continue;
    await conn.query(
      `INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, 0, NULL, NOW(), NOW())`,
      [nid(), u.id, nid().slice(0, 32), daysAgo(-7), `10.0.0.${i + 1}`, 'Mozilla/5.0 demo']
    );
  }
  // token_oauth_clients：1 条
  await conn.query(
    `INSERT INTO token_oauth_clients (client_id, client_secret_hash, name, permissions, grant_types, access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at)
     VALUES ('demo-client', ?, 'Demo OAuth Client', CAST('["read","write"]' AS JSON), CAST('["authorization_code","refresh_token"]' AS JSON), 3600, 604800, 'active', NOW(), NOW())
     ON DUPLICATE KEY UPDATE name = VALUES(name)`,
    ['$' + nid().slice(0, 32)] // placeholder hash
  );
  log('  · token refresh_markers / revoked_jtis / refresh_tokens / oauth_clients seeded');
}

// ---------- summary ----------
async function summarize(conn) {
  const tables = [
    'iam_users', 'iam_tenants', 'iam_org_units', 'iam_roles', 'iam_permissions',
    'iam_role_permissions', 'iam_user_roles', 'iam_user_org_units', 'iam_menus',
    'iam_login_logs', 'iam_refresh_tokens',
    'ntf_templates', 'ntf_announcements', 'ntf_user_projection',
    'audit_login_logs', 'audit_operation_logs', 'audit_api_access_logs', 'audit_events',
    'file_files', 'file_presign_urls',
    'cfg_namespaces', 'cfg_config_items', 'cfg_config_versions',
    'token_refresh_markers', 'token_revoked_jtis', 'token_oauth_clients',
  ];
  for (const t of tables) {
    try {
      const [rows] = await conn.query(`SELECT COUNT(*) c FROM ${t}`);
    const c = rows[0]?.c;
      log(`  ${t.padEnd(28)} ${c}`);
    } catch (e) {
      // skip
    }
  }

  log('\n=== login accounts ===');
  const codeToOrgName = { HQ: '总部', ENG: '研发部', PROD: '产品部', SALES: '销售部' };
  const placeholders = USERS.map(() => '?').join(',');
  const orderFields = USERS.map(() => '?').join(',');
  const [urows] = await conn.query(
    `SELECT username, nickname, email, roles, org_code FROM (
       SELECT u.username, u.nickname, u.email,
              GROUP_CONCAT(DISTINCT r.code ORDER BY r.code SEPARATOR ',') AS roles,
              MAX(CASE WHEN uou.is_primary = 1 THEN ou.code END) AS org_code
         FROM iam_users u
         LEFT JOIN iam_user_roles ur ON ur.user_id = u.id
         LEFT JOIN iam_roles r ON r.id = ur.role_id
         LEFT JOIN iam_user_org_units uou ON uou.user_id = u.id
         LEFT JOIN iam_org_units ou ON ou.id = uou.org_unit_id
        WHERE u.deleted_at IS NULL AND u.username IN (${placeholders})
        GROUP BY u.id, u.username, u.nickname, u.email
     ) AS t
     ORDER BY FIELD(username, ${orderFields})`,
    [...USERS.map((u) => u.username), ...USERS.map((u) => u.username)]
  );
  const w = (s, n) => String(s).padEnd(n);
  log(`  ${w('username', 10)} ${w('password', 10)} ${w('nickname', 12)} ${w('roles', 28)} ${w('org', 8)} email`);
  log(`  ${'-'.repeat(82)}`);
  for (const r of urows) {
    log(`  ${w(r.username, 10)} ${w('Demo@123', 10)} ${w(r.nickname || '', 12)} ${w(r.roles || '-', 28)} ${w(codeToOrgName[r.org_code] || r.org_code || '-', 8)} ${r.email || ''}`);
  }
}

main().catch((e) => {
  console.error('[demo-data] FAILED:', e.message);
  console.error(e.stack);
  process.exit(1);
});