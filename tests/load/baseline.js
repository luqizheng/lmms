import http from 'k6/http';
import { check, sleep } from 'k6';

// ---------------------------------------------------------------------------
// 配置
// ---------------------------------------------------------------------------
const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';

// ---------------------------------------------------------------------------
// 场景定义：ramp → sustain → ramp-down
// ---------------------------------------------------------------------------
export const options = {
  stages: [
    { duration: '2m', target: 50 },   // 2 分钟内爬升至 50 VUs
    { duration: '5m', target: 50 },   // 5 分钟持续 50 VUs
    { duration: '2m', target: 0 },    // 2 分钟平滑退出
  ],

  thresholds: {
    // p99 < 500ms
    'http_req_duration{scenario:default}': ['p(99)<500'],
    // 错误率 < 1%
    'http_req_failed{scenario:default}': ['rate<0.01'],
  },

  // 可选摘要趋势输出
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// ---------------------------------------------------------------------------
// 默认函数 — k6 对每个 VU 循环执行
// ---------------------------------------------------------------------------
export default function () {
  const res = http.get(`${BASE_URL}/health`, {
    tags: { scenario: 'default' },
  });

  check(res, {
    'health status is 200': (r) => r.status === 200,
  });

  // 每个 VU 迭代之间短暂停顿，模拟真实请求间隔
  sleep(1);
}
