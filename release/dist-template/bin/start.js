#!/usr/bin/env node
/**
 * start.js — 启动所有容器（含 healthcheck 等待）
 * 等价于原 release/dist-template/bin/start.sh
 *
 * 镜像已通过 node bin/load-images.js 加载，直接 `docker compose up`。
 */
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

// 启动所有容器
console.log('[start] → docker compose up -d');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml up -d', { stdio: 'inherit' });

// 等待关键服务健康
console.log('[start] 等待关键服务健康（最多 120s）...');
// 数据库方言：.env 的 DATABASE_URL scheme 决定首选等待目标（PG 模板的 compose
// 已把 postgres 提为默认、mysql 降为 profile，故 mysql 不会作为常驻容器存在）。
const dbService = /^DATABASE_URL="?postgres:/m.test(fs.readFileSync(path.join(ROOT, '.env'), 'utf8'))
  ? 'postgres'
  : 'mysql';
const services = [dbService, 'consul', 'redis', 'rabbitmq', 'api-gateway'];
for (const svc of services) {
  let healthy = false;
  for (let i = 1; i <= 60; i++) {
    try {
      const state = execSync(
        `docker inspect --format='{{.State.Health.Status}}' "coder-micro-services-${svc}"`,
        { stdio: 'pipe' },
      ).toString().trim();
      if (state === 'healthy') {
        console.log(`[start]   ✓ ${svc} healthy`);
        healthy = true;
        break;
      }
      if (state === 'not-found') {
        console.log(`[start]   ✗ ${svc} 未找到（容器名可能不匹配）`);
        break;
      }
    } catch {
      console.log(`[start]   ✗ ${svc} 未找到（容器名可能不匹配）`);
      break;
    }
    if (i === 60) {
      console.log(`[start]   ✗ ${svc} 健康检查超时`);
    }
    execSync('sleep 2', { stdio: 'pipe' });
  }
}

console.log('[start] ✓ 启动完成');
console.log('[start] 验证：curl http://localhost:3000/health');
try {
  execSync('curl -fsS http://localhost:3000/health', { stdio: 'inherit' });
  console.log(' → OK');
} catch {
  console.log(' → 失败（服务可能仍在启动）');
}
