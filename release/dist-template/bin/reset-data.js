#!/usr/bin/env node
/**
 * reset-data.js — 清除所有数据 volumes 并重新初始化
 * 等价于原 release/dist-template/bin/reset-data.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

console.log('[reset-data] → 停止所有容器并清除数据卷...');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml down -v', { stdio: 'inherit' });
console.log('[reset-data] ✓ 数据卷已清除');
console.log('[reset-data] → 重新启动...');
execSync(`node "${path.join(ROOT, 'bin', 'start.js')}"`, { stdio: 'inherit' });
