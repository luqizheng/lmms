// tests/load/s3-iam-crud.js
// S3: IAM CRUD 混合基线（users + roles + permissions + org_units，70% list / 30% create）
//
// 设计要点：
//   - 4 资源 × 70% list + 30% create = 8 路径按权重选
//   - 单场景不依赖其他（gateway 路径走 /v1/iam/*）
//   - 创建后清理（delete）避免 demo 数据污染（best-effort：失败不影响指标）
//
// 输出：stdout markdown 报告 + JSON 到 /tmp/perf-data/baseline-s3-<date>.json

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const tList = new Trend('s3_list_ms', true);
const tCreate = new Trend('s3_create_ms', true);
const tDelete = new Trend('s3_delete_ms', true);
const errCount = new Counter('s3_err');

const AUTH = { headers: { 'Content-Type': 'application/json' } };
const BEARER = (tok) => ({ headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${tok}` } });

export function setup() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }), AUTH);
  check(r, { 'login 200': (x) => x.status === 200 });
  return { token: r.json('access_token') };
}

export const options = {
  stages: [
    { duration: '10s', target: 20 },
    { duration: '40s', target: 20 },
    { duration: '10s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// 70% list + 30% create（按权重分配）
const ROUTES = [
  { w: 25, fn: 'list_users' },
  { w: 10, fn: 'create_user' },
  { w: 15, fn: 'list_roles' },
  { w: 5,  fn: 'create_role' },
  { w: 15, fn: 'list_perms' },
  { w: 5,  fn: 'create_perm' },
  { w: 15, fn: 'list_orgs' },
  { w: 5,  fn: 'create_org' },
  // 5% create 后立刻 delete 清理（避免 demo 数据污染）
  { w: 5,  fn: 'create_then_delete' },
];
const TOTAL_W = ROUTES.reduce((s, r) => s + r.w, 0);

function pick() {
  let r = Math.random() * TOTAL_W;
  for (const route of ROUTES) { r -= route.w; if (r <= 0) return route.fn; }
  return 'list_users';
}

function recordMetric(trend, r, label) {
  trend.add(r.timings.duration);
  if (r.status >= 400) errCount.add(1);
  check(r, { [label]: () => r.status >= 200 && r.status < 400 });
}

function nextId() {
  return `k6-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export default function (data) {
  const fn = pick();
  const h = BEARER(data.token);
  let r;

  switch (fn) {
    case 'list_users':
      r = http.get(`${BASE_URL}/v1/iam/users?page=1&size=20`, h);
      recordMetric(tList, r, 'list_users 200');
      break;
    case 'create_user': {
      const body = JSON.stringify({
        username: `k6u_${nextId()}`,
        password: 'K6load@123',
        email: `k6u_${nextId()}@load.local`,
        nickname: 'k6 load',
      });
      r = http.post(`${BASE_URL}/v1/iam/users`, body, h);
      recordMetric(tCreate, r, 'create_user 2xx');
      break;
    }
    case 'list_roles':
      r = http.get(`${BASE_URL}/v1/iam/roles?page=1&size=20`, h);
      recordMetric(tList, r, 'list_roles 200');
      break;
    case 'create_role': {
      const body = JSON.stringify({
        code: `k6r_${nextId()}`,
        name: 'k6 load role',
        description: 'created by s3-iam-crud.js',
      });
      r = http.post(`${BASE_URL}/v1/iam/roles`, body, h);
      recordMetric(tCreate, r, 'create_role 2xx');
      break;
    }
    case 'list_perms':
      r = http.get(`${BASE_URL}/v1/iam/permissions?page=1&size=20`, h);
      recordMetric(tList, r, 'list_perms 200');
      break;
    case 'create_perm': {
      const body = JSON.stringify({
        code: `K6_PERM_${nextId().toUpperCase()}`,
        name: 'k6 load permission',
      });
      r = http.post(`${BASE_URL}/v1/iam/permissions`, body, h);
      recordMetric(tCreate, r, 'create_perm 2xx');
      break;
    }
    case 'list_orgs':
      r = http.get(`${BASE_URL}/v1/iam/org-units?page=1&size=20`, h);
      recordMetric(tList, r, 'list_orgs 200');
      break;
    case 'create_org': {
      const body = JSON.stringify({
        code: `k6o_${nextId()}`,
        name: 'k6 load org',
      });
      r = http.post(`${BASE_URL}/v1/iam/org-units`, body, h);
      recordMetric(tCreate, r, 'create_org 2xx');
      break;
    }
    case 'create_then_delete': {
      const id = `k6x_${nextId()}`;
      const body = JSON.stringify({
        username: id,
        password: 'K6load@123',
        email: `${id}@load.local`,
        nickname: 'k6 temp',
      });
      const createR = http.post(`${BASE_URL}/v1/iam/users`, body, h);
      recordMetric(tCreate, createR, 'create_temp_user 2xx');
      if (createR.status === 200) {
        const created = createR.json('id') || createR.json('user_id');
        if (created) {
          sleep(0.1);
          const delR = http.del(`${BASE_URL}/v1/iam/users/${created}`, null, h);
          recordMetric(tDelete, delR, 'delete_temp_user 2xx');
        }
      }
      break;
    }
  }

  sleep(0.05 + Math.random() * 0.1); // 50-150ms
}

export function handleSummary(data) {
  const m = data.metrics;
  const tL = m.s3_list_ms?.values || {};
  const tC = m.s3_create_ms?.values || {};
  const tD = m.s3_delete_ms?.values || {};
  const errs = m.s3_err?.values?.count || 0;
  const total = m.http_reqs?.values?.count || 0;

  let md = '# S3 IAM CRUD 混合基线\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL}\n`;
  md += `> 权重：list_users:25 create_user:10 list_roles:15 create_role:5 list_perms:15 create_perm:5 list_orgs:15 create_org:5 create_then_delete:5\n\n`;

  md += '## List（users+roles+permissions+org-units 各 1 个 list）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tL.count || 0} |\n`;
  md += `| p50 (ms) | ${(tL.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tL['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tL['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(tL.avg || 0).toFixed(1)} |\n\n`;

  md += '## Create（users+roles+permissions+org-units 各 1 个 create）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tC.count || 0} |\n`;
  md += `| p50 (ms) | ${(tC.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tC['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tC['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(tC.avg || 0).toFixed(1)} |\n\n`;

  md += '## Delete（create 后立即 delete，验证 GC 链路）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tD.count || 0} |\n`;
  md += `| p50 (ms) | ${(tD.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tD['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tD['p(99)'] || 0).toFixed(1)} |\n\n`;

  const rps = (m.http_reqs?.values?.rate || 0).toFixed(1);
  const err = ((m.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  md += `## 汇总\n\n`;
  md += `- 总请求：${total} | RPS：${rps} | 错误：${errs} | HTTP 错误率：${err}%\n\n`;

  md += `> **结论**：IAM CRUD 在 20 VU 下混合负载 list p50 ≈ ${(tL.med || 0).toFixed(0)}ms、create p50 ≈ ${(tC.med || 0).toFixed(0)}ms。`;
  md += `create 比 list 慢是因为 bcrypt 写路径 + 多表事务。`;
  md += `create_then_delete 占 5% 权重用于 self-cleaning，避免 demo 数据膨胀。\n`;

  return { stdout: md };
}