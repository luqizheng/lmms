#!/usr/bin/env node
// Phase 10 G3 — 跨服务 schema state 端点联邦健康检查。
//
// Prereqs:
//   - infrastructure/docker-compose-infra.yml infra is up (mysql + redis + consul)
//   - infrastructure/docker-compose-services.yml services are up (api-gateway + 7 个业务服务)
//
// 调用 7 个服务的 GET /<svc>/internal/migrations/state 端点:
//   - 验证 HTTP 200 + JSON 结构
//   - 验证 in_sync=true（所有 migration 已应用）
//   - 验证 current_version 是预期的
//   - 验证 extra 为空（理想 schema drift 标志）
//
// Usage:
//   node scripts/check-migration-state.js
//   (exits 0 on all-pass, 1 on any-fail)

const http = require('node:http');
const { chalk } = require('./lib/ui.js');

// 默认端口映射 (来自 AGENTS.md / docker-compose-services.yml)
const SERVICES = [
  { name: 'iam-service',          port: 8081, path: '/iam/internal/migrations/state',   expected: { dialect: 'mysql',        minMigrations: 5 } },
  { name: 'notification-service', port: 8083, path: '/ntf/internal/migrations/state',   expected: { dialect: 'mysql',        minMigrations: 3 } },
  { name: 'token-service',        port: 8082, path: '/token/internal/migrations/state', expected: { dialect: 'mysql',        minMigrations: 1 } },
  { name: 'audit-service',        port: 8084, path: '/audit/internal/migrations/state', expected: { dialect: 'mysql',        minMigrations: 1 } },
  { name: 'config-service',       port: 8085, path: '/cfg/internal/migrations/state',   expected: { dialect: 'mysql',        minMigrations: 2 } },
  { name: 'file-service',         port: 8086, path: '/file/internal/migrations/state',  expected: { dialect: 'mysql',        minMigrations: 2 } },
  { name: 'prue_backend',         port: 8087, path: '/prue/internal/migrations/state',  expected: { dialect: 'mysql+sqlite', minMigrations: 1 } },
];

const HOST = process.env.E2E_HOST || 'localhost';
const TIMEOUT_MS = 5000;

function get(port, path) {
  return new Promise((resolve, reject) => {
    const req = http.request(
      { host: HOST, port, path, method: 'GET', timeout: TIMEOUT_MS },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          const body = Buffer.concat(chunks).toString('utf8');
          resolve({ status: res.statusCode, body });
        });
      },
    );
    req.on('timeout', () => req.destroy(new Error(`timeout ${TIMEOUT_MS}ms`)));
    req.on('error', (e) => reject(e));
    req.end();
  });
}

function ok(label) {
  return `${chalk.green('✓')} ${label}`;
}
function fail(label, detail) {
  return `${chalk.red('✗')} ${label}\n   ${detail}`;
}

async function checkService(svc) {
  const result = { svc: svc.name, checks: [], passed: true };
  let resp;
  try {
    resp = await get(svc.port, svc.path);
  } catch (e) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: HTTP request failed`, e.message));
    return result;
  }

  if (resp.status !== 200) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: HTTP ${resp.status}`, resp.body));
    return result;
  }
  result.checks.push(ok(`${svc.name}: HTTP 200`));

  let body;
  try {
    body = JSON.parse(resp.body);
  } catch (e) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: invalid JSON`, e.message));
    return result;
  }

  // 检查 service 字段
  if (body.service !== svc.name) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: service field mismatch`, `expected "${svc.name}", got "${body.service}"`));
  } else {
    result.checks.push(ok(`${svc.name}: service=${body.service}`));
  }

  // 检查 dialect 字段
  if (body.dialect !== svc.expected.dialect) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: dialect mismatch`, `expected "${svc.expected.dialect}", got "${body.dialect}"`));
  } else {
    result.checks.push(ok(`${svc.name}: dialect=${body.dialect}`));
  }

  // 检查 applied 数量
  const appliedCount = (body.applied || []).length;
  if (appliedCount < svc.expected.minMigrations) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: applied count too low`, `expected ≥${svc.expected.minMigrations}, got ${appliedCount}`));
  } else {
    result.checks.push(ok(`${svc.name}: applied count=${appliedCount}`));
  }

  // 检查 in_sync
  if (body.in_sync !== true) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: not in_sync`, `missing=${JSON.stringify(body.missing)} extra=${JSON.stringify(body.extra)}`));
  } else {
    result.checks.push(ok(`${svc.name}: in_sync=true`));
  }

  // 检查 current_version
  if (!body.current_version) {
    result.passed = false;
    result.checks.push(fail(`${svc.name}: current_version is null`, `applied=${JSON.stringify(body.applied)}`));
  } else {
    result.checks.push(ok(`${svc.name}: current_version=${body.current_version}`));
  }

  return result;
}

async function main() {
  console.log(`Phase 10 G3 — schema state 联邦健康检查`);
  console.log(`Host: ${HOST}`);
  console.log(`Services: ${SERVICES.length}\n`);

  const results = await Promise.all(SERVICES.map(checkService));

  for (const r of results) {
    console.log(`\n[${r.svc}]`);
    for (const line of r.checks) console.log('  ' + line);
  }

  const failed = results.filter((r) => !r.passed);
  console.log('');
  if (failed.length === 0) {
    console.log(chalk.green(`✓ all ${SERVICES.length} services in_sync`));
    process.exit(0);
  } else {
    console.log(chalk.red(`✗ ${failed.length}/${SERVICES.length} services failed checks`));
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});