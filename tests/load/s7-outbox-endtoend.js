// tests/load/s7-outbox-endtoend.js
// S7: announcement publish 端点基线（HTTP 段）
//
// 设计：用户决策 S7a 端到端。但 RabbitMQ mgt 端口 15672 在生产 compose 没暴露到宿主机，
//       所以 k6 内部不直接拉 mgt API。脚本只测 HTTP publish 端点本身。
//
// 端到端（队列深度变化）由外部 shell 用 `docker exec rabbitmqadmin` 在测试前后各采一次，
// 写入 baseline JSON 供 PR-impact diff 使用。详见 docs-site/perf-baseline.md §5.2。
//
// 输出：stdout markdown 报告

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const tPublish = new Trend('s7_publish_ms', true);
const publishOk = new Counter('s7_publish_ok');
const publishErr = new Counter('s7_publish_err');
const publishConflict = new Counter('s7_publish_409');

const BEARER = (tok) => ({ headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${tok}` } });

export function setup() {
  // 1. login
  const loginR = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });
  if (loginR.status !== 200) throw new Error(`login failed: ${loginR.status}`);
  const token = loginR.json('access_token');
  const h = BEARER(token);

  // 2. 预创建 announcement（draft 状态），给 VU publish 用
  // ⚠️ notification 服务 internal 限流 60 req/min per identity（digua_common_ratelimit 默认）
  //    setup 阶段只创建 ≤ 15 个 + 每 1.5s sleep，避开限流（限流窗口 60s 内最多 60 个 create）
  const ids = [];
  const preCreateCount = 15;
  for (let i = 0; i < preCreateCount; i++) {
    const now = new Date().toISOString();
    const exp = new Date(Date.now() + 86400000 * 30).toISOString();
    const r = http.post(`${BASE_URL}/v1/notification/announcements`, JSON.stringify({
      title: `k7-pre-${__VU || 0}-${i}`,
      body: 'pre-created for s7',
      publish_at: now,
      expire_at: exp,
      pinned: false,
      popover: false,
      recipients: [{ scope_type: 'role', scope_id: '1' }],
    }), h);
    if (r.status === 200 || r.status === 201) {
      const id = r.json('id') || r.json('announcement_id');
      if (id) ids.push(id);
    } else {
      console.log(`setup create ${i} failed status=${r.status} body=${r.body.slice(0,150)}`);
    }
    sleep(1.2); // 缓冲：限流 60/min = 1 req/s；1.2s 间隔留余量
  }
  if (ids.length === 0) throw new Error('no announcement pre-created');
  console.log(`setup prepared ${ids.length} announcement ids (target ${preCreateCount})`);
  return { token, preparedIds: ids };
}

export const options = {
  stages: [
    { duration: '5s', target: 2 },   // 1-2 VU：避开 60/60 限流
    { duration: '30s', target: 2 },  // 30s × 2 VU × ~1 req/s = ~60 reqs
    { duration: '5s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.10'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export default function (data) {
  const h = BEARER(data.token);
  // 随机选一个 draft announcement publish
  const id = data.preparedIds[Math.floor(Math.random() * data.preparedIds.length)];

  const r = http.post(`${BASE_URL}/v1/notification/announcements/${id}/publish`, '{}', h);
  tPublish.add(r.timings.duration);

  if (r.status === 200 || r.status === 201) {
    publishOk.add(1);
    check(r, { 'publish 2xx': () => true });
  } else if (r.status === 409) {
    // 重复 publish 是预期内的业务冲突（同一 announcement 只能 publish 一次）
    publishConflict.add(1);
    check(r, { 'publish 409 (已发布)': () => true });
  } else {
    publishErr.add(1);
    check(r, { 'publish fail': () => false });
    const errSeen = publishErr.values?.count || 0;
    if (errSeen <= 5) {
      console.log(`FAIL status=${r.status} id=${id} body=${r.body.slice(0,200)}`);
    }
  }

  sleep(0.8 + Math.random() * 0.4); // 800-1200ms 避开 60/min 限流
}

export function handleSummary(data) {
  const m = data.metrics;
  const tP = m.s7_publish_ms?.values || {};
  const ok = m.s7_publish_ok?.values?.count || 0;
  const conf = m.s7_publish_409?.values?.count || 0;
  const err = m.s7_publish_err?.values?.count || 0;
  const total = ok + conf + err;

  let md = '# S7 announcement publish 基线（HTTP 段）\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL}\n`;
  md += `> 完整端到端（HTTP → outbox → sidecar → RabbitMQ）由 shell 侧 docker exec rabbitmqadmin 测量\n\n`;

  md += '## HTTP publish 端点（POST /v1/notification/announcements/{id}/publish）\n\n';
  md += '| 指标 | 值 |\n|------|---|\n';
  md += `| 请求数 | ${tP.count || 0} |\n`;
  md += `| p50 (ms) | ${(tP.med || 0).toFixed(1)} |\n`;
  md += `| p95 (ms) | ${(tP['p(95)'] || 0).toFixed(1)} |\n`;
  md += `| p99 (ms) | ${(tP['p(99)'] || 0).toFixed(1)} |\n`;
  md += `| avg (ms) | ${(tP.avg || 0).toFixed(1)} |\n\n`;

  md += '## 响应分布\n\n';
  md += '| 状态 | 数量 | 占比 |\n|------|------|------|\n';
  md += `| 2xx (success) | ${ok} | ${total > 0 ? (ok/total*100).toFixed(1) : 0}% |\n`;
  md += `| 409 (重复 publish) | ${conf} | ${total > 0 ? (conf/total*100).toFixed(1) : 0}% |\n`;
  md += `| 其他错误 | ${err} | ${total > 0 ? (err/total*100).toFixed(1) : 0}% |\n`;
  md += `| **总计** | **${total}** | 100% |\n\n`;

  md += '## 端到端说明\n\n';
  md += '- **HTTP publish** 立即返回（写 outbox 表 + Redis 标记）；\n';
  md += '- **sidecar** `ntf_outbox_publisher` 周期轮询 outbox 表投到 RabbitMQ；\n';
  md += '- **RabbitMQ 队列** `notification.iam-events` 接收事件并被 audit-service 等消费者消费。\n\n';
  md += '完整端到端延迟 = HTTP publish 延迟 + sidecar 轮询周期（默认 ~1s）+ RabbitMQ 消费延迟。\n';
  md += '**测量方法**：测试前后用 `docker exec infrastructure-rabbitmq-1 rabbitmqadmin list queues name messages` 看队列深度变化；\n';
  md += '**目标**：每秒 sidecar 投递 ≥ HTTP publish 速率（即 outbox 不堆积）。\n';

  return { stdout: md };
}