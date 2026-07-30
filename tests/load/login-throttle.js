import http from 'k6/http';
import { check } from 'k6';
import { Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const okCount = new Counter('login_ok');
const rateLimitCount = new Counter('login_429');
const otherErrCount = new Counter('login_err');

export const options = {
  scenarios: {
    // Phase 1: 3/s (below limit)
    p1_3rps: {
      executor: 'constant-arrival-rate',
      rate: 3, timeUnit: '1s', duration: '30s',
      preAllocatedVUs: 5,
      exec: 'doLogin',
      tags: { phase: '3rps' },
    },
    // Phase 2: 5/s (at limit)
    p2_5rps: {
      executor: 'constant-arrival-rate',
      rate: 5, timeUnit: '1s', duration: '30s',
      preAllocatedVUs: 8,
      exec: 'doLogin',
      tags: { phase: '5rps' },
      startTime: '35s',
    },
    // Phase 3: 7/s (over limit, burst may absorb)
    p3_7rps: {
      executor: 'constant-arrival-rate',
      rate: 7, timeUnit: '1s', duration: '30s',
      preAllocatedVUs: 10,
      exec: 'doLogin',
      tags: { phase: '7rps' },
      startTime: '1m10s',
    },
    // Phase 4: 10/s (well over limit + burst)
    p4_10rps: {
      executor: 'constant-arrival-rate',
      rate: 10, timeUnit: '1s', duration: '30s',
      preAllocatedVUs: 12,
      exec: 'doLogin',
      tags: { phase: '10rps' },
      startTime: '1m45s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.80'], // 宽松，因为限流会导致大量 429
  },
};

export function doLogin() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });

  if (r.status === 200) {
    okCount.add(1);
    check(r, { 'login 200': (x) => true });
  } else if (r.status === 429) {
    rateLimitCount.add(1);
    check(r, { 'login 429 (rate limited)': (x) => true });
  } else {
    otherErrCount.add(1);
    check(r, { 'login err': (x) => false });
  }
}

export function handleSummary(data) {
  const m = data.metrics;
  const ok = m.login_ok?.values?.count || 0;
  const rl = m.login_429?.values?.count || 0;
  const err = m.login_err?.values?.count || 0;
  const total = ok + rl + err;

  let md = '# Login 限流复核\n\n';
  md += `> 测试：admin / Demo@123，constant-arrival-rate 分 4 阶段\n\n`;
  md += '| 阶段 | 目标速率 | 200 OK | 429 限流 | 其他错误 | 总计 |\n';
  md += '|------|---------|--------|----------|----------|------|\n';

  // 从 k6 默认 summary 中估计各阶段分布
  md += `| P1 3rps | 3/s | — | — | — | — |\n`;
  md += `| P2 5rps | 5/s | — | — | — | — |\n`;
  md += `| P3 7rps | 7/s | — | — | — | — |\n`;
  md += `| P4 10rps | 10/s | — | — | — | — |\n`;
  md += `\n> 总计: OK=${ok} 429=${rl} err=${err} total=${total}\n`;
  md += `> 限流比例: ${total > 0 ? (rl/total*100).toFixed(1) : 0}%\n`;

  return { stdout: md };
}
