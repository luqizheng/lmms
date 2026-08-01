#!/usr/bin/env node
/**
 * status.js — 列出所有容器运行状态
 * 等价于原 release/dist-template/bin/status.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

execSync('docker compose --env-file .env -f infrastructure/docker-compose.yml ps --format "table {{.Name}}\\t{{.Status}}\\t{{.Ports}}"', { stdio: 'inherit' });
