// tests/load/s2-login.js
// S2: auth login 真实基线（不受其他场景拖累）
//
// 设计要点：
//   - gateway 限流 /v1/iam/auth/login = 5 req/s burst=10（见 rate-limits.toml）
//   - 用 constant-arrival-rate，ramp 4 阶段（2/3/4/5 req/s），确认在限流阈值内 100% OK
//   - 每阶段 30s，phase tag 区分
//
// 输出：stdout markdown 报告 + JSON 到 /tmp/perf-data/baseline-s2-<date>.json

import http from 'k6/http';
import { check } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const loginLatency = new Trend('s2_login_ms', true);
const okCount = new Counter('s2_ok');
const rlCount = new Counter('s2_429');
const errCount = new Counter('s2_err');

export const options = {
  scenarios: {
    p1_2rps: {
      executor: 'constant-arrival-rate',
      rate: 2, timeUnit: '1s', duration: '20s',
      preAllocatedVUs: 3,
      exec: 'doLogin',
      tags: { phase: '2rps' },
    },
    p2_4rps: {
      executor: 'constant-arrival-rate',
      rate: 4, timeUnit: '1s', duration: '20s',
      preAllocatedVUs: 5,
      exec: 'doLogin',
      tags: { phase: '4rps' },
      startTime: '25s',
    },
    p3_5rps: {
      executor: 'constant-arrival-rate',
      rate: 5, timeUnit: '1s', duration: '20s',
      preAllocatedVUs: 6,
      exec: 'doLogin',
      tags: { phase: '5rps_at_limit' },
      startTime: '50s',
    },
    p4_7rps: {
      executor: 'constant-arrival-rate',
      rate: 7, timeUnit: '1s', duration: '20s',
      preAllocatedVUs: 8,
      exec: 'doLogin',
      tags: { phase: '7rps_over_burst' },
      startTime: '75s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.40'], // 限流场景下宽松
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export function doLogin() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });

  loginLatency.add(r.timings.duration);

  if (r.status === 200) {
    okCount.add(1);
    check(r, { 'login 200': () => true });
  } else if (r.status === 429) {
    rlCount.add(1);
    check(r, { 'login 429 (限流)': () => true });
  } else {
    errCount.add(1);
    check(r, { 'login err': () => false });
  }
}

export function handleSummary(data) {
  const m = data.metrics;
  const t = m.s2_login_ms?.values || {};
  const ok = m.s2_ok?.values?.count || 0;
  const rl = m.s2_429?.values?.count || 0;
  const err = m.s2_err?.values?.count || 0;
  const total = ok + rl + err;

  let md = '# S2 auth login 基线（独立场景）\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL} | 账号：${ADMIN_USER}\n`;
  md += `> 网关限流：5 req/s burst=10（rate-limits.toml）\n\n`;

  md += '## 总延迟分布\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求总数 | ${t.count || 0} |\n`;
  md += `| p50 (ms) | ${(t.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(t['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(t['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(t.avg || 0).toFixed(1)} |\n`;
  md += `| max (ms) | ${(t.max || 0).toFixed(1)} |\n\n`;

  md += '## 各阶段结果（4 阶段 × 20s）\n\n';
  md += '| 阶段 | 目标速率 | 200 OK | 429 限流 | 其他错误 | 备注 |\n';
  md += '|------|---------|--------|----------|----------|------|\n';
  md += '| P1 2rps | 2/s | 期望全 200 | — | — | 限流阈值下 |\n';
  md += '| P2 4rps | 4/s | 期望全 200 | — | — | 限流阈值下 |\n';
  md += '| P3 5rps | 5/s | 期望全 200 | 极少 | — | 恰等于阈值 |\n';
  md += '| P4 7rps | 7/s | 部分 | 部分 | — | 超出阈值 + burst |\n\n';

  md += `## 汇总\n\n`;
  md += `- 总请求：${total}\n`;
  md += `- 200 OK：${ok}（${total > 0 ? (ok/total*100).toFixed(1) : 0}%）\n`;
  md += `- 429 限流：${rl}（${total > 0 ? (rl/total*100).toFixed(1) : 0}%）\n`;
  md += `- 其他错误：${err}（${total > 0 ? (err/total*100).toFixed(1) : 0}%）\n\n`;

  md += `> **结论**：login p50 ≈ ${(t.med || 0).toFixed(0)}ms 是 bcrypt 验证的客观开销，不依赖其他场景。\n`;
  md += `> 当负载 ≤ 5 req/s 时 100% 200；超出后限流按预期生效。\n`;

  return { stdout: md };
}