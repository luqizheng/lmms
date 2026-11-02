'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectTemplateFiles, buildComposeTemplate } = require('../lib/compose-template.js');

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-'));
  const files = [
    'docker-compose.yml',
    'docker-compose-infra.yml',
    'docker-compose-services.yml',
    'base-images.json',
    '.env.prod.example',
    'README.md',
    'config/prometheus/prometheus.yml',
    'config/rabbitmq/rabbitmq.conf',
    'docker/healthcheck.sh',
    'mysql-replica/Dockerfile',
    'scripts/deploy-remote.sh',
    // excluded
    '.env',
    'AGENTS.md',
    'STATE.md',
    'LOOP.md',
    'loop-budget.md',
    'loop-run-log.md',
    '.grok/session.json',
    'vendor/swagger-ui.zip',
  ];
  for (const rel of files) {
    const p = path.join(dir, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, 'x', 'utf-8');
  }
  return dir;
}

test('collectTemplateFiles keeps compose layout and excludes secrets/session files', () => {
  const dir = fixture();
  const files = collectTemplateFiles(dir);
  const rel = files.map((f) => f.rel).sort();
  assert.ok(rel.includes('docker-compose.yml'));
  assert.ok(rel.includes('docker-compose-infra.yml'));
  assert.ok(rel.includes('base-images.json'));
  assert.ok(rel.includes('.env.prod.example'), 'whitelisted dotfile kept');
  assert.ok(!rel.includes('README.md'), 'lmms-owned docs excluded — consumers write their own');
  assert.ok(rel.includes(path.join('config', 'prometheus', 'prometheus.yml').replaceAll('\\', '/')));
  assert.ok(rel.includes(path.join('scripts', 'deploy-remote.sh').replaceAll('\\', '/')));
  assert.ok(!rel.includes('.env'), 'real secrets excluded');
  assert.ok(!rel.includes('AGENTS.md'));
  assert.ok(!rel.includes('STATE.md'));
  assert.ok(!rel.includes('LOOP.md'));
  assert.ok(!rel.some((f) => /^loop-/.test(f)), 'loop notes excluded');
  assert.ok(!rel.some((f) => f.startsWith('.grok')), 'dot dirs excluded');
  assert.ok(!rel.some((f) => f.startsWith('vendor')), 'build-time vendor excluded');
});

test('buildComposeTemplate copies files and writes TEMPLATE_INFO.json', () => {
  const src = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out-'));
  const result = buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });

  assert.match(path.basename(result.dir), /coder-compose-template-1\.6\.0$/);
  const copied = fs.readFileSync(path.join(result.dir, 'docker-compose.yml'), 'utf-8');
  assert.equal(copied, 'x');
  assert.ok(fs.existsSync(path.join(result.dir, 'config', 'rabbitmq', 'rabbitmq.conf')));
  assert.ok(!fs.existsSync(path.join(result.dir, '.env')), 'secret never lands in the bundle');

  const info = JSON.parse(fs.readFileSync(path.join(result.dir, 'TEMPLATE_INFO.json'), 'utf-8'));
  assert.equal(info.version, '1.6.0');
  assert.equal(info.fileCount, result.fileCount);
  assert.ok(info.generatedAt);
  assert.ok(result.fileCount >= 8);
});

test('buildComposeTemplate overwrites a previous bundle of the same version', () => {
  const src = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out2-'));
  buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });
  const second = buildComposeTemplate({ infraDir: src, outRoot, version: '1.6.0' });
  assert.ok(fs.existsSync(path.join(second.dir, 'TEMPLATE_INFO.json')));
});

// ---------------------------------------------------------------------------
// PG 方言变体（2026-11-03）
// ---------------------------------------------------------------------------

const yaml = require('js-yaml');
const { transformForPg, DIALECTS } = require('../lib/compose-template.js');

test('pg dialect: postgres promoted, mysql demoted to profile', () => {
  const infra = {
    services: {
      consul: { image: 'consul' },
      mysql: { image: 'mysql:8.0', healthcheck: {} },
      postgres: { image: 'postgres:16', profiles: ['pg'], healthcheck: {} },
    },
    volumes: { mysql_data: {}, postgres_data: {} },
  };
  const out = yaml.load(transformForPg('docker-compose-infra.yml', yaml.dump(infra)));
  assert.equal(out.services.postgres.profiles, undefined, 'postgres 必须摘除 profiles 成为默认');
  assert.deepEqual(out.services.mysql.profiles, ['mysql'], 'mysql 必须降级为可选 profile');
});

test('pg dialect: depends_on mysql flipped, token DB_TYPE postgres, replica removed', () => {
  const doc = yaml.load(`
services:
  config-service:
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
  token-service:
    environment:
      TOKEN_DATABASE_DB_TYPE: "mysql"
    depends_on:
      mysql:
        condition: service_healthy
  mysql-replica:
    image: coder/mysql-replica:8.0
volumes:
  mysql_replica_data: {}
`);
  const out = yaml.load(transformForPg('docker-compose-services.yml', yaml.dump(doc)));
  assert.deepEqual(Object.keys(out.services['config-service'].depends_on), ['postgres', 'consul']);
  assert.equal(out.services['token-service'].environment.TOKEN_DATABASE_DB_TYPE, 'postgres');
  assert.equal(out.services['mysql-replica'], undefined, 'replica 必须整体移除');
  assert.equal(out.volumes.mysql_replica_data, undefined, 'replica 卷必须移除');
});

test('pg dialect: env DATABASE_URL becomes postgres scheme (idempotent)', () => {
  const once = transformForPg('.env', 'DATABASE_URL="mysql://app:apppass@mysql:3306/superpowers"\nPOSTGRES_HOST=postgres\n');
  assert.match(once, /^DATABASE_URL="postgres:\/\//m);
  assert.match(once, /POSTGRES_HOST=postgres/);
  const twice = transformForPg('.env', once);
  assert.equal(once, twice, '重复变换必须幂等');
});

test('pg dialect: prod overlay gets postgres ports override (textual)', () => {
  const text = 'services:\n  mysql:\n    ports: !override []\n';
  const out = transformForPg('docker-compose.prod.yml', text);
  assert.match(out, /postgres:\n\s+ports: !override \[\]/);
  assert.ok(out.includes('mysql'), '原 mysql 收敛块保留');
});

test('pg dialect: non-transformable files returned as null (copy as-is)', () => {
  assert.equal(transformForPg('base-images.json', '{}'), null);
  assert.equal(transformForPg('config/prometheus/prometheus.yml', 'global: {}\n'), null);
});

test('pg bundle: dir suffix -pg, replica/init dirs excluded, TEMPLATE_INFO.dialect', () => {
  const dir = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out-'));
  const result = buildComposeTemplate({ infraDir: dir, outRoot, version: '9.9.9', dialect: 'pg' });
  assert.ok(result.dir.endsWith('coder-compose-template-9.9.9-pg'));
  assert.ok(!fs.existsSync(path.join(result.dir, 'mysql-replica')), 'replica 目录不进 PG 包');
  const info = JSON.parse(fs.readFileSync(path.join(result.dir, 'TEMPLATE_INFO.json'), 'utf8'));
  assert.equal(info.dialect, 'postgres');
  // mysql 默认模板目录名不带后缀，两者并存互不覆盖
  const mysqlResult = buildComposeTemplate({ infraDir: dir, outRoot, version: '9.9.9' });
  assert.ok(fs.existsSync(mysqlResult.dir));
  assert.ok(!mysqlResult.dir.endsWith('-pg'));
  const mysqlInfo = JSON.parse(fs.readFileSync(path.join(mysqlResult.dir, 'TEMPLATE_INFO.json'), 'utf8'));
  assert.equal(mysqlInfo.dialect, 'mysql');
});

test('invalid dialect rejected', () => {
  assert.throws(() => buildComposeTemplate({ infraDir: '.', outRoot: '.', version: '1.0.0', dialect: 'oracle' }), /invalid dialect/);
  assert.ok(DIALECTS.has('mysql') && DIALECTS.has('pg'));
});
