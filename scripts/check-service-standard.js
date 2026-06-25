#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const SERVICES_DIR = path.join(__dirname, '..', 'services');
const REQUIRED = [
  'Dockerfile',
  'AGENTS.md',
  'README.md',
  'tests/ 目录',
  '/health 端点',
  '/metrics 端点',
  'migrations/ 目录',
  'Swagger 配置（SWAGGER_ENABLED 开关）',
];

function parseExemptions(content) {
  const m = content.match(/## 豁免\s*\n([\s\S]*?)(?:\n##|$)/);
  if (!m) return new Set();
  return new Set(
    m[1]
      .split('\n')
      .map(l => l.replace(/^[-*]\s*/, '').trim())
      .filter(Boolean)
  );
}

function fileExists(p) {
  return fs.existsSync(p);
}

function readSafe(p) {
  return fileExists(p) ? fs.readFileSync(p, 'utf-8') : '';
}

function hasRoute(serviceDir, route) {
  const candidates = [
    path.join(serviceDir, 'src/main.rs'),
    path.join(serviceDir, 'src/routes/mod.rs'),
    path.join(serviceDir, 'src/lib.rs'),
  ];
  const regex = new RegExp(`\\.route\\(\\s*"${route}"`);
  return candidates.some(p => regex.test(readSafe(p)));
}

function hasSwagger(serviceDir) {
  const cargo = readSafe(path.join(serviceDir, 'Cargo.toml'));
  if (!/utoipa-swagger-ui/.test(cargo)) return false;
  const main = readSafe(path.join(serviceDir, 'src/main.rs'));
  return /SWAGGER_ENABLED/.test(main);
}

function checkComponent(serviceDir, component) {
  switch (component) {
    case 'Dockerfile':
      return fileExists(path.join(serviceDir, 'Dockerfile'));
    case 'AGENTS.md':
      return fileExists(path.join(serviceDir, 'AGENTS.md'));
    case 'README.md':
      return fileExists(path.join(serviceDir, 'README.md'));
    case 'tests/ 目录':
      return (
        fileExists(path.join(serviceDir, 'tests')) &&
        fs.readdirSync(path.join(serviceDir, 'tests')).some(f => f.endsWith('.rs'))
      );
    case '/health 端点':
      return hasRoute(serviceDir, '/health');
    case '/metrics 端点':
      return hasRoute(serviceDir, '/metrics');
    case 'migrations/ 目录':
      return fileExists(path.join(serviceDir, 'migrations'));
    case 'Swagger 配置（SWAGGER_ENABLED 开关）':
      return hasSwagger(serviceDir);
    default:
      return false;
  }
}

function main() {
  const services = fs
    .readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  console.log('=== 服务规范检查报告 ===\n');
  let failed = false;

  for (const svc of services) {
    const svcDir = path.join(SERVICES_DIR, svc);
    const agentsPath = path.join(svcDir, 'AGENTS.md');
    console.log(`\n## ${svc}`);

    if (!fileExists(agentsPath)) {
      console.log('⚠️  缺失规则文件: AGENTS.md');
      failed = true;
      continue;
    }

    const exemptions = parseExemptions(fs.readFileSync(agentsPath, 'utf-8'));

    for (const comp of REQUIRED) {
      if (exemptions.has(comp)) {
        console.log(`◯ 豁免: ${comp}`);
        continue;
      }
      if (checkComponent(svcDir, comp)) {
        console.log(`✓ ${comp}`);
      } else {
        console.log(`⚠️  缺失: ${comp}`);
        failed = true;
      }
    }
  }

  if (failed) {
    console.log('\n❌ 服务规范检查未通过');
    process.exit(1);
  }
  console.log('\n✅ 服务规范检查通过');
}

main();
