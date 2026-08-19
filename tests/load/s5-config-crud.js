// tests/load/s5-config-crud.js
// S5: config CRUD 混合基线（namespaces + items）
//
// ⚠️ 已知 bug：list config_items 端点（GET /config/item）当前实现下 100% 返回 500，
//    已记入 baseline（perf-baseline.md §5 v2）等待修复。脚本避开该端点，
//    仅覆盖 list_namespaces + create_namespace + create_item + delete。
//
// 设计要点：
//   - 70% list/read（namespace list + namespace get）+ 30% create（namespace + item）
//   - list items 已禁用（500），改测 list namespaces 提升权重
//   - create_then_delete_* 占 20% 用于 self-cleaning

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const tList = new Trend('s5_list_ms', true);
const tGet = new Trend('s5_get_ms', true);
const tCreate = new Trend('s5_create_ms', true);
const tUpdate = new Trend('s5_update_ms', true);
const tDelete = new Trend('s5_delete_ms', true);
const errCount = new Counter('s5_err');

const BEARER = (tok) => ({ headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${tok}` } });

export function setup() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });
  check(r, { 'login 200': (x) => x.status === 200 });
  return { token: r.json('access_token') };
}

export const options = {
  stages: [
    { duration: '10s', target: 8 },
    { duration: '40s', target: 8 },
    { duration: '10s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.10'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max', 'count'],
};

const ROUTES = [
  { w: 50, fn: 'list_namespaces' },
  { w: 10, fn: 'get_first_namespace' },
  { w: 10, fn: 'create_namespace' },
  { w: 10, fn: 'create_item' },
  { w: 10, fn: 'create_then_delete_namespace' },
  { w: 10, fn: 'create_then_delete_item' },
];
const TOTAL_W = ROUTES.reduce((s, r) => s + r.w, 0);

function pick() {
  let r = Math.random() * TOTAL_W;
  for (const route of ROUTES) { r -= route.w; if (r <= 0) return route.fn; }
  return 'list_namespaces';
}

function recordMetric(trend, r, label) {
  trend.add(r.timings.duration);
  if (r.status >= 400) errCount.add(1);
  check(r, { [label]: () => r.status >= 200 && r.status < 400 });
}

let cachedNamespaceId = null;

export default function (data) {
  const fn = pick();
  const h = BEARER(data.token);
  let r;

  switch (fn) {
    case 'list_namespaces':
      r = http.get(`${BASE_URL}/v1/config/namespace?page=1&size=20`, h);
      recordMetric(tList, r, 'list_namespaces 200');
      if (r.status === 200) {
        try {
          const body = r.json();
          if (body.items && body.items.length > 0) cachedNamespaceId = body.items[0].id;
        } catch (e) {}
      }
      break;
    case 'get_first_namespace':
      if (cachedNamespaceId) {
        r = http.get(`${BASE_URL}/v1/config/namespace/${cachedNamespaceId}`, h);
        recordMetric(tGet, r, 'get_namespace 200');
      }
      break;
    case 'create_namespace': {
      const body = JSON.stringify({
        app: `k6s5-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        env: 'perf',
        profile: 'baseline',
        description: 's5 config namespace load',
      });
      r = http.post(`${BASE_URL}/v1/config/namespace`, body, h);
      recordMetric(tCreate, r, 'create_namespace 2xx');
      break;
    }
    case 'create_item': {
      const key = `k6.s5.${Date.now()}.${Math.random().toString(36).slice(2, 6)}`;
      const body = JSON.stringify({
        namespace_id: cachedNamespaceId || 'TIiqDU19B2YsywMjU5r92',
        key,
        value: 'k5',
        value_type: 'string',
        description: 's5 config item load',
      });
      r = http.post(`${BASE_URL}/v1/config/item`, body, h);
      recordMetric(tCreate, r, 'create_item 2xx');
      break;
    }
    case 'create_then_delete_namespace': {
      const body = JSON.stringify({
        app: `k6x-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        env: 'perf',
        profile: 'tmp',
        description: 's5 cleanup',
      });
      const createR = http.post(`${BASE_URL}/v1/config/namespace`, body, h);
      recordMetric(tCreate, createR, 'create_tmp_namespace 2xx');
      if (createR.status === 200) {
        try {
          const created = createR.json('id');
          if (created) {
            sleep(0.1);
            const delR = http.del(`${BASE_URL}/v1/config/namespace/${created}`, null, h);
            recordMetric(tDelete, delR, 'delete_tmp_namespace 2xx');
          }
        } catch (e) {}
      }
      break;
    }
    case 'create_then_delete_item': {
      const key = `k6x.s5.${Date.now()}.${Math.random().toString(36).slice(2, 6)}`;
      const body = JSON.stringify({
        namespace_id: cachedNamespaceId || 'TIiqDU19B2YsywMjU5r92',
        key,
        value: 'tmp',
        value_type: 'string',
      });
      const createR = http.post(`${BASE_URL}/v1/config/item`, body, h);
      recordMetric(tCreate, createR, 'create_tmp_item 2xx');
      if (createR.status === 200) {
        try {
          const created = createR.json('id');
          if (created) {
            sleep(0.1);
            const delR = http.del(`${BASE_URL}/v1/config/item/${created}`, null, h);
            recordMetric(tDelete, delR, 'delete_tmp_item 2xx');
          }
        } catch (e) {}
      }
      break;
    }
  }

  sleep(0.05 + Math.random() * 0.1);
}

export function handleSummary(data) {
  const m = data.metrics;
  const fmt = (t) => !t || !t.values.count ? '—' : `${t.values.count} | p50=${(t.values.med||0).toFixed(1)} p95=${(t.values['p(95)']||0).toFixed(1)} p99=${(t.values['p(99)']||0).toFixed(1)} avg=${(t.values.avg||0).toFixed(1)}`;
  const errs = m.s5_err?.values?.count || 0;
  const total = m.http_reqs?.values?.count || 0;

  let md = '# S5 config CRUD 混合基线\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL}\n`;
  md += `> ⚠️ 已知 bug：list config_items 端点 100% 500 query failed，已记入 baseline。脚本避开该端点。\n`;
  md += `> 权重：list_namespaces:50 get:10 create_ns:10 create_item:10 create+del_ns:10 create+del_item:10\n\n`;
  md += '## 各操作延迟\n\n';
  md += '| 操作 | 请求数 / p50 / p95 / p99 / avg (ms) |\n|------|------|\n';
  md += `| list (namespaces) | ${fmt(m.s5_list_ms)} |\n`;
  md += `| get (namespace) | ${fmt(m.s5_get_ms)} |\n`;
  md += `| create (ns/item) | ${fmt(m.s5_create_ms)} |\n`;
  md += `| update | ${fmt(m.s5_update_ms)} |\n`;
  md += `| delete | ${fmt(m.s5_delete_ms)} |\n\n`;

  const rps = (m.http_reqs?.values?.rate || 0).toFixed(1);
  const err = ((m.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  md += `## 汇总\n\n- 总请求：${total} | RPS：${rps} | 错误：${errs} | HTTP 错误率：${err}%\n\n`;

  md += `> **结论**：config namespaces 列表读路径 p50 ≈ ${((m.s5_list_ms?.values?.med) || 0).toFixed(0)}ms；`;
  md += `namespace 创建 p50 ≈ ${((m.s5_create_ms?.values?.med) || 0).toFixed(0)}ms。`;
  md += `create_then_delete_* 占 20% 用于 self-cleaning。`;
  md += `**list items 端点有 bug，待修复后补测。**\n`;

  return { stdout: md };
}