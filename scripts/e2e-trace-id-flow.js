#!/usr/bin/env node
// T1 traceparent 透传 e2e：用带 traceparent header 创建用户 → 断言 audit 表
// trace_id == traceparent 中的 trace-id。
//
// 与 scripts/e2e-event-flow.js 同模式：原生 fetch + execSync 进 mysql 容器，
// 无 Vitest/supertest 依赖，CI 一条命令即可跑。
//
// Prereqs:
//   - infrastructure docker compose 全栈（api-gateway + iam + audit + outbox-publisher）
//   - admin 用户 e2eadmin / P@ssw0rd!（tests/e2e globalSetup 注册）
//
// Usage:
//   node scripts/e2e-trace-id-flow.js
//   退出码 0=PASS，非 0=FAIL

const { execSync } = require('node:child_process');

const GATEWAY = process.env.E2E_BASE_URL || 'http://localhost:3000';
const ADMIN = process.env.E2E_ADMIN_USER || 'e2eadmin';
const PASS = process.env.E2E_ADMIN_PASS || 'P@ssw0rd!';
const MYSQL_CONTAINER = process.env.E2E_MYSQL_CONTAINER || 'infrastructure-mysql-1';
const TIMEOUT_MS = Number(process.env.E2E_TRACE_TIMEOUT_MS || 30_000);
const POLL_MS = 500;

async function httpJson(method, path, body, token, extraHeaders = {}) {
  const url = GATEWAY + path;
  const headers = { 'Content-Type': 'application/json', ...extraHeaders };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(url, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json;
  try { json = text ? JSON.parse(text) : {}; } catch { json = { _raw: text }; }
  return { status: res.status, body: json };
}

function mysqlScalar(sql) {
  const cmd = `docker exec ${MYSQL_CONTAINER} mysql -uroot -prootpass -N -B -e ${JSON.stringify(sql)} superpowers`;
  return execSync(cmd, { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'ignore'] }).trim();
}

function assert(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

async function main() {
  const traceId = 'aabbccddeeff00112233445566778899';
  const traceparent = `00-${traceId}-1122334455667788-01`;
  console.log(`[e2e-trace-id-flow] trace_id=${traceId}`);

  console.log('[e2e-trace-id-flow] login as admin');
  const login = await httpJson('POST', '/v1/iam/auth/login', { username: ADMIN, password: PASS });
  assert(login.status === 200 && login.body.access_token, `login failed: ${login.status} ${JSON.stringify(login.body)}`);
  const token = login.body.access_token;

  const username = `trace_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  console.log(`[e2e-trace-id-flow] create user ${username} with traceparent`);
  const create = await httpJson(
    'POST',
    '/v1/iam/users',
    { username, password: 'Trace1234!Trace1234!', nickname: 'trace-id-test' },
    token,
    { traceparent },
  );
  assert(
    create.status === 200 || create.status === 201,
    `create_user failed: ${create.status} ${JSON.stringify(create.body)}`,
  );
  const userId = create.body.id;
  console.log(`[e2e-trace-id-flow] user created id=${userId}`);

  console.log('[e2e-trace-id-flow] polling audit_events for trace_id match...');
  let auditTraceId = '';
  let lastErr = null;
  const deadline = Date.now() + TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      auditTraceId = mysqlScalar(
        `SELECT IFNULL(trace_id,'') \
         FROM audit_events \
         WHERE event_type='iam.user.created' \
           AND JSON_UNQUOTE(JSON_EXTRACT(payload_json,'$.user_id'))='${userId}' \
         ORDER BY id DESC LIMIT 1`,
      );
      if (auditTraceId) break;
    } catch (e) { lastErr = e; }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }
  if (lastErr) console.warn(`[e2e-trace-id-flow] mysqlScalar last error: ${lastErr.message || lastErr}`);

  assert(
    auditTraceId === traceId,
    `expected trace_id=${traceId} in audit trace_id column, got '${auditTraceId}'`,
  );

  console.log(`[e2e-trace-id-flow] PASS — audit trace_id=${auditTraceId} matches caller's W3C trace_id`);
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});