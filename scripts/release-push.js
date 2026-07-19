#!/usr/bin/env node
// scripts/release-push.js
//
// 给 9 个 micro-server 镜像打 :<short-sha> 与 :latest 两个 tag，push 到
// zhcoder-docker-registry.com:8000。镜像必须先由 build-docker.js 生成（裸 tag，
// 不带版本号）。
//
// 用法：
//   IMAGE_TAG=<sha> node scripts/release-push.js              # 9 服务全推
//   IMAGE_TAG=<sha> node scripts/release-push.js api-gateway   # 子集
//   node scripts/release-push.js                              # 默认取 git short-sha
//
// 行为：fail-fast — 任一 docker push 失败立即退出非零码，未推送的 tag 保持不变。

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const REGISTRY = 'zhcoder-docker-registry.com:8000';
const REPO = 'micro-server';
const ALL_SERVICES = [
  'api-gateway',
  'token-service',
  'iam-service',
  'notification-service',
  'file-service',
  'config-service',
  'audit-service',
  'prue-admin-backend',
  'frontend',
];

function run(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts });
  if (res.status !== 0) {
    console.error(`[fail] ${cmd} ${args.join(' ')} -> exit ${res.status}`);
    process.exit(res.status || 1);
  }
}

function getShortSha() {
  const res = spawnSync('git', ['rev-parse', '--short', 'HEAD'], {
    encoding: 'utf-8',
    shell: false,
  });
  if (res.status !== 0) {
    console.error('[fail] git rev-parse --short HEAD 失败 — 当前目录不是 git 仓库？');
    process.exit(1);
  }
  return res.stdout.trim();
}

function listTargets() {
  const args = process.argv.slice(2).filter(a => ALL_SERVICES.includes(a));
  return args.length > 0 ? args : ALL_SERVICES;
}

function pushService(name, sha) {
  const baseRef = `${REGISTRY}/${REPO}/${name}`;
  const shaTag = `${baseRef}:${sha}`;
  const latestTag = `${baseRef}:latest`;

  console.log(`\n[tag] ${name}: -> :${sha} & :latest`);
  run('docker', ['tag', baseRef, shaTag]);
  run('docker', ['tag', baseRef, latestTag]);

  console.log(`[push] ${shaTag}`);
  run('docker', ['push', shaTag]);
  console.log(`[push] ${latestTag}`);
  run('docker', ['push', latestTag]);
}

function main() {
  const sha = process.env.IMAGE_TAG || getShortSha();
  const targets = listTargets();

  console.log(`[release] IMAGE_TAG=${sha}`);
  console.log(`[release] targets (${targets.length}): ${targets.join(', ')}`);

  const start = Date.now();
  for (const svc of targets) {
    pushService(svc, sha);
  }
  const sec = ((Date.now() - start) / 1000).toFixed(1);
  console.log(`\n[done] 成功推送 ${targets.length} 服务 × 2 tag，耗时 ${sec}s`);
}

main();
