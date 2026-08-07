#!/usr/bin/env node
/**
 * start.js — 启动所有容器（含 healthcheck 等待）
 * 等价于原 release/dist-template/bin/start.sh
 *
 * flag：
 *   --deploy    跳过 build-frontend（部署包用户场景：frontend 已预构建在镜像中）
 *
 * 默认 dev/二次开发场景会从 ./frontend 源码构建 nginx 镜像；deploy 场景跳过这一步
 * 直接 `docker compose up` 因为 frontend image 已通过 node bin/load-images.js 加载。
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

const args = process.argv.slice(2);
const isDeploy = args.includes('--deploy');

if (isDeploy) {
  console.log('[start] --deploy 模式：跳过 build-frontend（镜像已预构建并 load 进本地 docker）');
} else {
  // 1. 确保 frontend 镜像存在（dev/二次开发场景）
  execSync(`node "${path.join(ROOT, 'bin', 'build-frontend.js')}"`, { stdio: 'inherit' });
}

// 2. 启动所有容器
console.log('[start] → docker compose up -d');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml up -d', { stdio: 'inherit' });

// 3. 等待关键服务健康
console.log('[start] 等待关键服务健康（最多 120s）...');
const services = ['mysql', 'consul', 'redis', 'rabbitmq', 'api-gateway'];
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
