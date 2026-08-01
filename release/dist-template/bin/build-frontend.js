#!/usr/bin/env node
/**
 * build-frontend.js — 从 ./frontend 源码构建 nginx 镜像
 * BuildKit cache mount 命中后增量秒级；镜像存在则跳过
 *
 * 等价于原 release/dist-template/bin/build-frontend.sh
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const REGISTRY = process.env.REGISTRY || 'zhcoder-docker-registry.com:8000';

// IMAGE_TAG: 优先环境变量，其次从 .env 读取
let TAG = process.env.IMAGE_TAG || '';
if (!TAG) {
  const envFile = path.join(ROOT, '.env');
  if (existsSync(envFile)) {
    try {
      const content = require('fs').readFileSync(envFile, 'utf-8');
      const match = content.match(/^IMAGE_TAG=(.+)$/m);
      if (match) TAG = match[1].trim();
    } catch { /* ignore */ }
  }
}
TAG = TAG || 'v1.0.0';

const IMG = `${REGISTRY}/micro-server/frontend:${TAG}`;

// 检查镜像是否已存在
try {
  execSync(`docker image inspect "${IMG}"`, { stdio: 'pipe' });
  console.log(`[build-frontend] ✓ ${IMG} 已存在，跳过 build`);
  process.exit(0);
} catch {
  // 镜像不存在，继续构建
}

console.log(`[build-frontend] → 构建 ${IMG}`);
console.log('[build-frontend]   首次 build 约需 1-2 分钟（pnpm install），后续命中 cache 秒级');

// 选择构建上下文：优先 dev 仓库 frontend/，其次分发包 frontend/
const devFrontend = path.resolve(ROOT, '..', '..', 'frontend');
const distFrontend = path.resolve(ROOT, 'frontend');
const BUILD_CONTEXT = existsSync(devFrontend) ? devFrontend : distFrontend;
console.log(`[build-frontend] → 构建上下文: ${BUILD_CONTEXT}`);

const args = [
  'build',
  '--build-arg', 'BUILDKIT_INLINE_CACHE=1',
  '--build-arg', `VITE_API_BASE=${process.env.VITE_API_BASE || '/api'}`,
  '--build-arg', `VITE_DISABLE_CAPTCHA=${process.env.VITE_DISABLE_CAPTCHA || 'true'}`,
  '--tag', IMG,
  BUILD_CONTEXT,
];

try {
  execSync(`docker ${args.join(' ')}`, {
    stdio: 'inherit',
    env: { ...process.env, DOCKER_BUILDKIT: '1' },
  });
} catch {
  console.error(`[build-frontend] ✗ 构建失败: ${IMG}`);
  process.exit(1);
}

console.log(`[build-frontend] ✓ 构建完成: ${IMG}`);

if (process.env.PUSH === '1') {
  console.log(`[build-frontend] → push ${IMG}`);
  try {
    execSync(`docker push "${IMG}"`, { stdio: 'inherit' });
  } catch {
    console.error(`[build-frontend] ✗ push 失败: ${IMG}`);
    process.exit(1);
  }
}
