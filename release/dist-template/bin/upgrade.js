#!/usr/bin/env node
/**
 * upgrade.js — 保留 volumes 就地升级镜像
 * 用法: node bin/upgrade.js <新版本号>
 * 等价于原 release/dist-template/bin/upgrade.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

const NEW_TAG = process.argv[2];
if (!NEW_TAG) {
  console.log('用法: node bin/upgrade.js <新版本号>  例如: node bin/upgrade.js v1.1.0');
  process.exit(1);
}

console.log(`[upgrade] → 拉取新镜像 tag=${NEW_TAG} ...`);
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml pull', { stdio: 'inherit' });

console.log('[upgrade] → 重启服务（volumes 保留）...');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml up -d', { stdio: 'inherit' });

console.log(`[upgrade] ✓ 升级完成: ${NEW_TAG}`);
console.log('[upgrade] 验证: node bin/verify.js');
