// tests/load/s6-file-upload.js
// S6: file upload 基线（multipart + metadata）
//
// 设计要点：
//   - file upload 是 multipart/form-data，比 JSON POST 重
//   - 70% 上传（10KB / 100KB / 1MB 三档随机）+ 30% 列表读
//   - 上传后立即 delete 清理（avoid 磁盘/DB 膨胀）
//
// 输出：stdout markdown 报告

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const ADMIN_USER = __ENV.ADMIN_USER || 'admin';
const ADMIN_PASS = __ENV.ADMIN_PASS || 'Demo@123';

const tList = new Trend('s6_list_ms', true);
const tUpload10K = new Trend('s6_upload_10k_ms', true);
const tUpload100K = new Trend('s6_upload_100k_ms', true);
const tUpload1M = new Trend('s6_upload_1m_ms', true);
const tDelete = new Trend('s6_delete_ms', true);
const errCount = new Counter('s6_err');

const BEARER = (tok) => ({ headers: { Authorization: `Bearer ${tok}` } });

export function setup() {
  const r = http.post(`${BASE_URL}/v1/iam/auth/login`,
    JSON.stringify({ username: ADMIN_USER, password: ADMIN_PASS }),
    { headers: { 'Content-Type': 'application/json' } });
  check(r, { 'login 200': (x) => x.status === 200 });
  return { token: r.json('access_token') };
}

export const options = {
  stages: [
    { duration: '10s', target: 10 },  // 文件上传较重，VU 少一些
    { duration: '40s', target: 10 },
    { duration: '10s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max', 'count'],
};

// 权重：list 30% / upload-10k 20% / upload-100k 20% / upload-1m 10% / cleanup 20%
const ROUTES = [
  { w: 30, fn: 'list' },
  { w: 20, fn: 'upload_10k' },
  { w: 20, fn: 'upload_100k' },
  { w: 10, fn: 'upload_1m' },
  { w: 20, fn: 'upload_then_delete' },
];
const TOTAL_W = ROUTES.reduce((s, r) => s + r.w, 0);

function pick() {
  let r = Math.random() * TOTAL_W;
  for (const route of ROUTES) { r -= route.w; if (r <= 0) return route.fn; }
  return 'list';
}

function recordMetric(trend, r, label) {
  trend.add(r.timings.duration);
  if (r.status >= 400) errCount.add(1);
  check(r, { [label]: () => r.status >= 200 && r.status < 400 });
}

function makeFile(sizeBytes) {
  const data = randomString(sizeBytes);
  return http.file(data, `k6-${sizeBytes}.bin`, 'application/octet-stream');
}

// UploadFileMeta schema: {filename, content_type?, expires_in_seconds?, is_temporary?}
// multipart form-data: metadata (json string) + file (raw bytes)
function uploadBytes(sizeBytes, meta) {
  const data = randomString(sizeBytes);
  const fileObj = http.file(data, meta.filename, meta.content_type || 'application/octet-stream');
  return {
    metadata: JSON.stringify(meta),
    file: fileObj,
  };
}

export default function (data) {
  const fn = pick();
  const h = BEARER(data.token);
  let r;

  switch (fn) {
    case 'list':
      r = http.get(`${BASE_URL}/v1/file/list?page=1&size=10`, h);
      recordMetric(tList, r, 'list 200');
      break;
    case 'upload_10k':
    case 'upload_100k':
    case 'upload_1m': {
      const sizeMap = {
        upload_10k: { bytes: 10 * 1024, trend: tUpload10K },
        upload_100k: { bytes: 100 * 1024, trend: tUpload100K },
        upload_1m: { bytes: 1024 * 1024, trend: tUpload1M },
      };
      const { bytes, trend } = sizeMap[fn];
      const meta = { filename: `k6-${bytes}.bin`, content_type: 'application/octet-stream', is_temporary: false };
      const formData = uploadBytes(bytes, meta);
      r = http.post(`${BASE_URL}/v1/file/upload`, formData, h);
      recordMetric(trend, r, `${fn} 2xx`);
      break;
    }
    case 'upload_then_delete': {
      const meta = { filename: `k6-tmp-${Date.now()}.bin`, content_type: 'application/octet-stream', is_temporary: true };
      const formData = uploadBytes(10 * 1024, meta);
      const uploadR = http.post(`${BASE_URL}/v1/file/upload`, formData, h);
      recordMetric(tUpload10K, uploadR, 'upload_tmp 2xx');
      if (uploadR.status === 200 || uploadR.status === 201) {
        const fileId = uploadR.json('id') || uploadR.json('file_id');
        if (fileId) {
          sleep(0.1);
          const delR = http.del(`${BASE_URL}/v1/file/${fileId}`, null, h);
          recordMetric(tDelete, delR, 'delete_tmp 2xx');
        }
      }
      break;
    }
  }

  sleep(0.1 + Math.random() * 0.2); // 100-300ms（上传重，sleep 长点）
}

export function handleSummary(data) {
  const m = data.metrics;
  const fmt = (t) => !t || !t.values.count ? '—' : `${t.values.count} | p50=${(t.values.med||0).toFixed(1)} p95=${(t.values['p(95)']||0).toFixed(1)} p99=${(t.values['p(99)']||0).toFixed(1)} avg=${(t.values.avg||0).toFixed(1)}`;
  const errs = m.s6_err?.values?.count || 0;
  const total = m.http_reqs?.values?.count || 0;

  let md = '# S6 file upload 基线\n\n';
  md += `> 时间：${new Date().toISOString()} | 目标：${BASE_URL}\n`;
  md += `> 权重：list:30 upload-10k:20 upload-100k:20 upload-1m:10 upload+del:20\n\n`;
  md += '## 各操作延迟\n\n';
  md += '| 操作 | 请求数 / p50 / p95 / p99 / avg (ms) |\n|------|------|\n';
  md += `| list | ${fmt(m.s6_list_ms)} |\n`;
  md += `| upload 10KB | ${fmt(m.s6_upload_10k_ms)} |\n`;
  md += `| upload 100KB | ${fmt(m.s6_upload_100k_ms)} |\n`;
  md += `| upload 1MB | ${fmt(m.s6_upload_1m_ms)} |\n`;
  md += `| delete | ${fmt(m.s6_delete_ms)} |\n\n`;

  const rps = (m.http_reqs?.values?.rate || 0).toFixed(1);
  const err = ((m.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  md += `## 汇总\n\n- 总请求：${total} | RPS：${rps} | 错误：${errs} | HTTP 错误率：${err}%\n\n`;

  md += `> **结论**：file 列表读 p50 ≈ ${((m.s6_list_ms?.values?.med) || 0).toFixed(0)}ms。`;
  md += `上传延迟随 payload 增长：10k ≈ ${((m.s6_upload_10k_ms?.values?.med) || 0).toFixed(0)}ms / 100k ≈ ${((m.s6_upload_100k_ms?.values?.med) || 0).toFixed(0)}ms / 1MB ≈ ${((m.s6_upload_1m_ms?.values?.med) || 0).toFixed(0)}ms。`;
  md += `upload_then_delete 占 20% 用于 self-cleaning，避免存储膨胀。\n`;

  return { stdout: md };
}