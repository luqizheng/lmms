// tests/load/s4-notify-announce.js
// S4: notification announcement publish 端到端基线
//
// 设计要点：
//   - 用户决策 = S7a 端到端，所以这里也走 publish 完整路径
//   - S4 = 高频读取（list_mine），S7 = outbox publish（独立脚本）
//   - 70% list_mine + 30% 创建 announcement（不立即 publish，留给 S7）
//
// 输出：stdout markdown 报告

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const tList = new Trend('s4_list_mine_ms', true);
const tCreate = new Trend('s4_create_ms', true);
const errCount = new Counter('s4_err');

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
    { duration: '3s', target: 1 },   // 1 VU：notification 服务 internal 限流 60 req/min per identity
    { duration: '30s', target: 1 },  // 30s × 1 VU × ~1 req/s = ~30 reqs（限流阈值内）
    { duration: '3s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export default function (data) {
  const h = BEARER(data.token);
  const now = new Date().toISOString();
  const exp = new Date(Date.now() + 86400000 * 30).toISOString();

  // 70% list_mine（read-heavy）
  if (Math.random() < 0.7) {
    const r = http.get(`${BASE_URL}/v1/notification/announcements?page=1&size=20`, h);
    tList.add(r.timings.duration);
    if (r.status >= 400) errCount.add(1);
    check(r, { 'list_mine 200': () => r.status === 200 });
  } else {
    // 30% create announcement（draft 状态，不立即 publish）
    const body = JSON.stringify({
      title: `k6-s4-${__VU}-${__ITER}`,
      body: 's4-announce load',
      publish_at: now,
      expire_at: exp,
      pinned: false,
      popover: false,
      recipients: [{ scope_type: 'role', scope_id: '1' }],
    });
    const r = http.post(`${BASE_URL}/v1/notification/announcements`, body, h);
    tCreate.add(r.timings.duration);
    if (r.status >= 400) errCount.add(1);
    check(r, { 'create 2xx': () => r.status >= 200 && r.status < 300 });
  }

  sleep(0.8 + Math.random() * 0.4); // 800-1200ms（避开 notification 60/min 限流）
}

export function handleSummary(data) {
  const m = data.metrics;
  const tL = m.s4_list_mine_ms?.values || {};
  const tC = m.s4_create_ms?.values || {};
  const errs = m.s4_err?.values?.count || 0;
  const total = m.http_reqs?.values?.count || 0;

  let md = '# S4 notification announce 基线\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL}\n`;
  md += `> 权重：70% list_mine + 30% create announcement（draft）\n\n`;

  md += '## List mine（读路径）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tL.count || 0} |\n`;
  md += `| p50 (ms) | ${(tL.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tL['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tL['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(tL.avg || 0).toFixed(1)} |\n\n`;

  md += '## Create announcement（draft，写路径）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tC.count || 0} |\n`;
  md += `| p50 (ms) | ${(tC.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tC['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tC['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(tC.avg || 0).toFixed(1)} |\n\n`;

  const rps = (m.http_reqs?.values?.rate || 0).toFixed(1);
  const err = ((m.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  md += `## 汇总\n\n`;
  md += `- 总请求：${total} | RPS：${rps} | 错误：${errs} | HTTP 错误率：${err}%\n\n`;

  md += `> **结论**：notification 列表读 p50 ≈ ${(tL.med || 0).toFixed(0)}ms，写 announcement p50 ≈ ${(tC.med || 0).toFixed(0)}ms。`;
  md += `S7 outbox publish 端到端延迟见 s7-outbox-endtoend.js 报告。\n`;

  return { stdout: md };
}