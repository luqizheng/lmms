#!/usr/bin/env node
/**
 * upgrade.js — 保留 volumes 就地升级镜像
 * 用法: node bin/upgrade.js <新版本号>   例如: node bin/upgrade.js v1.1.0（1.1.0 亦可）
 *
 * 版本切换机制：
 *   分发包 compose 的镜像 tag 由打包时 transform-compose.js 写为 `${IMAGE_TAG:-<打包版本>}`。
 *   compose 插值优先级是 进程环境变量 > --env-file，因此本脚本：
 *     1) 把 .env 的 IMAGE_TAG= 行改写为新版本（持久化，后续 make up 沿用新版本）；
 *     2) 以 IMAGE_TAG=<no-v 版本> 环境变量执行 compose pull + up -d（volumes 保留）。
 */
const { execSync } = require('child_process');
const { existsSync, readFileSync, writeFileSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

const NEW_TAG = String(process.argv[2] || '').replace(/^v/, '');
if (!/^\d+\.\d+\.\d+$/.test(NEW_TAG)) {
  console.log('用法: node bin/upgrade.js <新版本号>  例如: node bin/upgrade.js v1.1.0');
  process.exit(1);
}

const ENV_FILE = path.join(ROOT, '.env');
if (!existsSync(ENV_FILE)) {
  console.error('[upgrade] ✗ .env 不存在 — 先执行 make secrets 初始化');
  process.exit(1);
}

const envText = readFileSync(ENV_FILE, 'utf-8');
const updatedEnv = /^IMAGE_TAG=.*$/m.test(envText)
  ? envText.replace(/^IMAGE_TAG=.*$/m, `IMAGE_TAG=${NEW_TAG}`)
  : (envText.endsWith('\n') || envText === '' ? envText : envText + '\n') + `IMAGE_TAG=${NEW_TAG}\n`;
writeFileSync(ENV_FILE, updatedEnv);
console.log(`[upgrade] ✓ .env IMAGE_TAG → ${NEW_TAG}`);

const composeEnv = { ...process.env, IMAGE_TAG: NEW_TAG };

console.log(`[upgrade] → 拉取新镜像 tag=${NEW_TAG} ...`);
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml pull', { stdio: 'inherit', env: composeEnv });

console.log('[upgrade] → 重启服务（volumes 保留）...');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml up -d', { stdio: 'inherit', env: composeEnv });

console.log(`[upgrade] ✓ 升级完成: ${NEW_TAG}`);
console.log('[upgrade] 验证: node bin/verify.js');
