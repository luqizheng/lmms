import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

// 按端点拆分延迟
const tHealth = new Trend('m1_health_ms');
const tJwks = new Trend('m1_jwks_ms');
const tUsers = new Trend('m1_users_ms');
const tConfig = new Trend('m1_config_ms');
const tAudit = new Trend('m1_audit_ms');
const tNotify = new Trend('m1_notify_ms');
const err429 = new Counter('m1_429_count');

export function setup() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });
  check(r, { 'login ok': (x) => x.status === 200 });
  return { token: r.json('access_token') };
}

export const options = {
  stages: [
    { duration: '30s', target: 60 },
    { duration: '2m', target: 60 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.15'],
    'm1_health_ms': ['p(99)<50'],
    'm1_users_ms': ['p(99)<200'],
    'm1_jwks_ms': ['p(99)<50'],
    'm1_config_ms': ['p(99)<200'],
    'm1_audit_ms': ['p(99)<500'],
    'm1_notify_ms': ['p(99)<500'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// 权重分配 (不包括 login，因为 login 限流)
const ROUTES = [
  { w: 20, fn: 'health' },
  { w: 30, fn: 'jwks' },
  { w: 30, fn: 'users' },
  { w: 10, fn: 'config' },
  { w: 5, fn: 'audit' },
  { w: 5, fn: 'notify' },
];
const TOTAL_W = ROUTES.reduce((s, r) => s + r.w, 0);

function pick() {
  let r = Math.random() * TOTAL_W;
  for (const route of ROUTES) { r -= route.w; if (r <= 0) return route.fn; }
  return 'health';
}

export default function (data) {
  const fn = pick();
  let res;
  switch (fn) {
    case 'health':
      res = http.get(`${BASE_URL}/health`);
      check(res, { 'h 200': (r) => r.status === 200 });
      tHealth.add(res.timings.duration);
      break;
    case 'jwks':
      res = http.get(`${BASE_URL}/.well-known/jwks.json`);
      check(res, { 'j 200': (r) => r.status === 200 });
      tJwks.add(res.timings.duration);
      break;
    case 'users':
      res = http.get(`${BASE_URL}/v1/iam/users?page=1&size=10`, {
        headers: { Authorization: `Bearer ${data.token}` },
      });
      check(res, { 'u 200': (r) => r.status === 200 });
      tUsers.add(res.timings.duration);
      break;
    case 'config':
      res = http.get(`${BASE_URL}/v1/config/namespace?page=1&size=10`, {
        headers: { Authorization: `Bearer ${data.token}` },
      });
      check(res, { 'c 200': (r) => r.status === 200 });
      tConfig.add(res.timings.duration);
      break;
    case 'audit':
      res = http.get(`${BASE_URL}/v1/audit/operation?page=1&size=10`, {
        headers: { Authorization: `Bearer ${data.token}` },
      });
      check(res, { 'a 200': (r) => r.status === 200 });
      tAudit.add(res.timings.duration);
      break;
    case 'notify': {
      const now = new Date().toISOString();
      const exp = new Date(Date.now() + 86400000 * 30).toISOString();
      res = http.post(`${BASE_URL}/v1/notification/announcements`,
        JSON.stringify({ title: `mix-${__VU}-${__ITER}`, body: 'm1', publish_at: now, expire_at: exp, pinned: false, popover: false, recipients: [{ scope_type: 'role', scope_id: '1' }] }),
        { headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${data.token}` } });
      check(res, { 'n 201': (r) => r.status === 201 });
      tNotify.add(res.timings.duration);
      break;
    }
  }
  if (res && res.status === 429) err429.add(1);
  sleep(0.15 + Math.random() * 0.2); // 150-350ms
}

export function handleSummary(data) {
  const m = data.metrics;
  const rows = [
    ['health', 'GET /health', m.m1_health_ms],
    ['jwks', 'GET /.well-known/jwks.json', m.m1_jwks_ms],
    ['users', 'GET /v1/iam/users', m.m1_users_ms],
    ['config', 'GET /v1/config/namespace', m.m1_config_ms],
    ['audit', 'GET /v1/audit/operation', m.m1_audit_ms],
    ['notify', 'POST /v1/notification/announcements', m.m1_notify_ms],
  ];
  let md = '# M1 混合负载报告\n\n';
  md += `> 时间：${new Date().toISOString()} | VU: 60 | 权重: health:20 jwks:30 users:30 config:10 audit:5 notify:5\n\n`;
  md += '| 端点 | 请求数 | p50 (ms) | p95 (ms) | p99 (ms) | avg (ms) |\n';
  md += '|------|--------|----------|----------|----------|----------|\n';
  for (const [n, ep, t] of rows) {
    if (!t || !t.values.count) { md += `| ${ep} | 0 | — | — | — | — |\n`; continue; }
    const c = t.values;
    md += `| ${ep} | ${c.count} | ${c.med.toFixed(1)} | ${(c['p(95)']||0).toFixed(1)} | ${(c['p(99)']||0).toFixed(1)} | ${c.avg.toFixed(1)} |\n`;
  }
  const rps = (m.http_reqs?.values?.rate || 0).toFixed(1);
  const err = ((m.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  const n429 = m.m1_429_count?.values?.count || 0;
  md += `\n> RPS: ${rps} req/s | 错误率: ${err}% | HTTP 429: ${n429} | 总数: ${m.http_reqs?.values?.count || 0}\n`;
  return { stdout: md };
}
