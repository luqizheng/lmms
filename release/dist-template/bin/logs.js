#!/usr/bin/env node
/**
 * logs.js — tail 实时日志
 * 用法: node bin/logs.js [服务名]
 * 等价于原 release/dist-template/bin/logs.sh
 */
const { execSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
process.chdir(ROOT);

const svc = process.argv[2] || '';
const cmd = svc
  ? `docker compose --env-file .env -f infrastructure/docker-compose.yml logs -f --tail=100 "${svc}"`
  : 'docker compose --env-file .env -f infrastructure/docker-compose.yml logs -f --tail=50';

execSync(cmd, { stdio: 'inherit' });
