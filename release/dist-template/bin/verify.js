#!/usr/bin/env node
/**
 * verify.js — 端到端冒烟测试（检查所有服务 HTTP 端口响应）
 * 等价于原 release/dist-template/bin/verify.sh
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

function check(name, url) {
  try {
    execSync(`curl -sfS "${url}"`, { stdio: 'pipe' });
    console.log(`  ✓ ${name}`);
    return true;
  } catch {
    console.log(`  ✗ ${name} (${url})`);
    return false;
  }
}

console.log('=== verify: HTTP 端点检查 ===');
let ok = 0, fail = 0;
const checks = [
  ['api-gateway',    'http://localhost:8080/health'],
  ['config-service', 'http://localhost:8081/health'],
  ['file-service',   'http://localhost:8082/health'],
  ['token-service',  'http://localhost:8083/health'],
  ['iam-service',    'http://localhost:8084/health'],
  ['audit-service',  'http://localhost:8085/health'],
  ['frontend',       'http://localhost:3000/'],
  ['prometheus',     'http://localhost:9090/-/healthy'],
  ['grafana',        'http://localhost:3001/api/health'],
];
for (const [name, url] of checks) {
  check(name, url) ? ok++ : fail++;
}

console.log('\n=== verify: Docker 容器状态 ===');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml ps --format "table {{.Name}}\\t{{.Status}}"', { stdio: 'inherit' });

console.log(`\n结果: ${ok} 通过, ${fail} 失败`);
process.exit(fail === 0 ? 0 : 1);
