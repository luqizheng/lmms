#!/usr/bin/env node
/**
 * stop.js — 停止所有容器（保留 volumes）
 * 等价于原 release/dist-template/bin/stop.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

console.log('[stop] → docker compose down');
execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml down', { stdio: 'inherit' });
console.log('[stop] ✓ 停止完成（volumes 已保留）');
