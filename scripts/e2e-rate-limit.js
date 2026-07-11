#!/usr/bin/env node
// 网关限流验证：连续/并发打请求 → 断言部分返回 429 + Retry-After。
// 与 scripts/e2e-event-flow.js 同模式：原生 fetch + 退出码 0=PASS。
//
// 验证两条限流规则（infrastructure/config/api-gateway/rate-limits.toml）：
//   [routes."/v1/iam/auth/login"]  requests_per_second = 5  burst = 10  （严格）
//   [defaults]                       requests_per_second = 100 burst = 200（普通 GET）
//
// 用例：
//   rl-auth-login-001: 15 次连续 login，前 10 通过（burst），后 5 个 429
//   rl-default-001:    250 次并发普通 GET，前 200 通过（burst），后 50 个 429
//
// Prereqs:
//   - infrastructure docker compose 全栈
//   - admin 用户 e2eadmin / P@ssw0rd!
//
// Usage:
//   node scripts/e2e-rate-limit.js
//   退出码 0=PASS，非 0=FAIL

const GATEWAY = process.env.E2E_BASE_URL || 'http://localhost:3000';
const ADMIN = process.env.E2E_ADMIN_USER || 'e2eadmin';
const PASS = process.env.E2E_ADMIN_PASS || 'P@ssw0rd!';

async function httpJson(method, path, body, token) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(GATEWAY + path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  let json;
  try { json = JSON.parse(await res.text()); } catch { json = null; }
  return {
    status: res.status,
    retryAfter: res.headers.get('retry-after'),
    body: json,
  };
}

function assert(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

async function rlAuthLogin() {
  console.log('[rl-auth-login-001] 连续 15 次 POST /v1/iam/auth/login');
  console.log('  期望：前 10 通过（burst），后续触发 429 + Retry-After');
  const results = [];
  for (let i = 0; i < 15; i++) {
    const r = await httpJson('POST', '/v1/iam/auth/login', { username: ADMIN, password: PASS });
    results.push(r);
  }
  const ok = results.filter((r) => r.status === 200).length;
  const rl = results.filter((r) => r.status === 429);
  const withRetryAfter = rl.filter((r) => r.retryAfter).length;
  console.log(`  → 通过: ${ok}/15，429: ${rl.length}，带 Retry-After: ${withRetryAfter}`);

  assert(ok >= 10, `burst=10 应至少 10 个 200，实际 ${ok}`);
  assert(rl.length >= 1,
    `应至少 1 个 429 触发，实际 ${rl.length}。可能揭示 bug: 限流 fail-open（Redis pipeline 解析 ZCARD 返回失败），见 libs/common-ratelimit/src/lib.rs`);
  assert(withRetryAfter === rl.length, `所有 429 应带 Retry-After 头（实际 ${withRetryAfter}/${rl.length}）`);
  return { ok, rl: rl.length };
}

async function rlDefault(token) {
  console.log('[rl-default-001] 并发 250 次 GET /v1/iam/users');
  console.log('  期望：前 200 通过（burst），后续触发 429 + Retry-After');
  console.log('  等待 5s 让 auth 限流桶恢复');
  await new Promise((r) => setTimeout(r, 5000));

  const promises = [];
  for (let i = 0; i < 250; i++) {
    promises.push(httpJson('GET', '/v1/iam/users?page=1&size=1', null, token));
  }
  const results = await Promise.all(promises);
  const ok = results.filter((r) => r.status === 200).length;
  const rl = results.filter((r) => r.status === 429);
  const withRetryAfter = rl.filter((r) => r.retryAfter).length;
  console.log(`  → 通过: ${ok}/250，429: ${rl.length}，带 Retry-After: ${withRetryAfter}`);

  assert(ok >= 200, `default burst=200 应至少 200 个 200，实际 ${ok}`);
  assert(rl.length >= 10,
    `应至少 10 个 429 触发（burst 耗尽），实际 ${rl.length}。可能揭示 bug: 限流 fail-open`);
  assert(withRetryAfter === rl.length, `所有 429 应带 Retry-After 头（实际 ${withRetryAfter}/${rl.length}）`);
  return { ok, rl: rl.length };
}

async function main() {
  console.log('=== 网关限流验证 ===');
  console.log(`Gateway: ${GATEWAY}`);

  console.log('admin login...');
  const login = await httpJson('POST', '/v1/iam/auth/login', { username: ADMIN, password: PASS });
  assert(login.status === 200, `admin login 失败: ${login.status} ${JSON.stringify(login.body)}`);
  const token = login.body.access_token;

  const a = await rlAuthLogin();
  const d = await rlDefault(token);

  console.log('---');
  console.log('rl-auth-login-001: 通过=' + a.ok + ', 429=' + a.rl);
  console.log('rl-default-001:    通过=' + d.ok + ', 429=' + d.rl);
  console.log('PASS — 网关限流按 rate-limits.toml 配置生效');
  process.exit(0);
}

main().catch((e) => { console.error(e); process.exit(1); });