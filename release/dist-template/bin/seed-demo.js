#!/usr/bin/env node
/**
 * seed-demo.js — 向 API 网关写入 demo 数据
 * 等价于原 release/dist-template/bin/seed-demo.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

// 加载 .env
try {
  const envContent = require('fs').readFileSync(path.join(ROOT, '.env'), 'utf-8');
  for (const line of envContent.split('\n')) {
    const m = line.match(/^(\w+)=(.*)$/);
    if (m && !m[1].startsWith('#')) process.env[m[1]] = m[2].trim();
  }
} catch { /* ignore */ }

const GATEWAY = process.env.API_GATEWAY_URL || 'http://localhost:8080';
console.log(`[seed-demo] → POST /api/v1/seed to ${GATEWAY}`);
try {
  execSync(
    `curl -sfS -X POST "${GATEWAY}/api/v1/seed" -H "Content-Type: application/json" -d '{"environment":"demo"}'`,
    { stdio: 'inherit' },
  );
  console.log('[seed-demo] ✓ demo 数据写入成功');
} catch {
  console.log('[seed-demo] ✗ demo 数据写入失败');
}
