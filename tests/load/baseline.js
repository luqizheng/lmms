import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const s1 = new Trend('S1_health_ms', true);
const s2 = new Trend('S2_login_ms', true);
const s3 = new Trend('S3_users_ms', true);
const s4 = new Trend('S4_jwks_ms', true);
const s5 = new Trend('S5_config_ms', true);
const s6 = new Trend('S6_audit_ms', true);
const s7 = new Trend('S7_notify_ms', true);

export function setup() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });
  check(r, { 'login 200': (x) => x.status === 200 });
  const token = r.json('access_token');
  for (let i = 0; i < 3; i++) {
    http.post(`${BASE_URL}/v1/iam/auth/login`,
      JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
      { headers: { 'Content-Type': 'application/json' } });
    sleep(0.3);
  }
  return { token };
}

export const options = {
  stages: [
    { duration: '15s', target: 10 },
    { duration: '45s', target: 10 },
    { duration: '15s', target: 0 },
  ],
  thresholds: { http_req_failed: ['rate<0.10'] },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

const SCENARIOS = [
  { name: 'S1_health', fn: (d) => { const r = http.get(`${BASE_URL}/health`); s1.add(r.timings.duration); return r.status === 200; }},
  { name: 'S2_login', fn: (d) => { const r = http.post(`${BASE_URL}/v1/iam/auth/login`, JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }), { headers: { 'Content-Type': 'application/json' } }); s2.add(r.timings.duration); return r.status === 200 || r.status === 401; }},
  { name: 'S3_users', fn: (d) => { const r = http.get(`${BASE_URL}/v1/iam/users?page=1&size=10`, { headers: { Authorization: `Bearer ${d.token}` } }); s3.add(r.timings.duration); return r.status === 200; }},
  { name: 'S4_jwks', fn: (d) => { const r = http.get(`${BASE_URL}/.well-known/jwks.json`); s4.add(r.timings.duration); return r.status === 200; }},
  { name: 'S5_config', fn: (d) => { const r = http.get(`${BASE_URL}/v1/config/namespace?page=1&size=10`, { headers: { Authorization: `Bearer ${d.token}` } }); s5.add(r.timings.duration); return r.status === 200; }},
  { name: 'S6_audit', fn: (d) => { const r = http.get(`${BASE_URL}/v1/audit/operation?page=1&size=10`, { headers: { Authorization: `Bearer ${d.token}` } }); s6.add(r.timings.duration); return r.status === 200; }},
  { name: 'S7_notify', fn: (d) => { const r = http.post(`${BASE_URL}/v1/notification/announcements`, JSON.stringify({ title: `k6-${__VU}-${__ITER}`, body: 'load', publish_at: new Date().toISOString(), expire_at: new Date(Date.now()+86400000*30).toISOString(), pinned: false, popover: false, recipients: [{ scope_type: 'role', scope_id: '1' }] }), { headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${d.token}` } }); s7.add(r.timings.duration); return r.status === 201; }},
];

let scenarioIdx = 0;
export default function (data) {
  const s = SCENARIOS[scenarioIdx % SCENARIOS.length];
  scenarioIdx++;
  const ok = s.fn(data);
  check(null, { [s.name]: () => ok });
  sleep(0.3);
}

export function handleSummary(data) {
  const m = data.metrics;
  const rows = [
    ['S1_health', 'GET /health', m.S1_health_ms],
    ['S2_login', 'POST /v1/iam/auth/login', m.S2_login_ms],
    ['S3_users', 'GET /v1/iam/users', m.S3_users_ms],
    ['S4_jwks', 'GET /.well-known/jwks.json', m.S4_jwks_ms],
    ['S5_config', 'GET /v1/config/namespace', m.S5_config_ms],
    ['S6_audit', 'GET /v1/audit/operation', m.S6_audit_ms],
    ['S7_notify', 'POST /v1/notification/announcements', m.S7_notify_ms],
  ];
  let md = '# k6 基线压测报告\n\n';
  md += `> 时间：${new Date().toISOString()}\n> 目标：${BASE_URL}\n> 账号：${ADMIN_USER}\n\n`;
  md += '| 场景 | 端点 | 请求数 | p50 (ms) | p95 (ms) | p99 (ms) | avg (ms) |\n';
  md += '|------|------|--------|----------|----------|----------|----------|\n';
  for (const [name, ep, trend] of rows) {
    if (!trend || trend.values.count === 0) { md += `| ${name} | ${ep} | 0 | — | — | — | — |\n`; continue; }
    const c = trend.values;
    md += `| ${name} | ${ep} | ${c.count} | ${c.med.toFixed(1)} | ${(c['p(95)']||0).toFixed(1)} | ${(c['p(99)']||0).toFixed(1)} | ${c.avg.toFixed(1)} |\n`;
  }
  md += `\n> 全局 RPS: ${(m.http_reqs?.values?.rate||0).toFixed(1)} req/s | 错误率: ${((m.http_req_failed?.values?.rate||0)*100).toFixed(2)}% | 总数: ${m.http_reqs?.values?.count||0}\n`;
  return { stdout: md };
}
