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
    // 合体入口 + prod 聚合片（根，2026-11-06 三组进自包含子目录）
    'docker-compose.yml',
    'docker-compose.prod.yml',
    // 三组自包含子目录：compose 各变体 + 组内 config 分区
    'infra/docker-compose-infra.yml',
    'infra/docker-compose-infra.dev.yml',
    'infra/docker-compose-infra.prod.yml',
    'infra/docker-compose-infra.remote.yml',
    'infra/.env.example',
    'infra/config/consul/connect.hcl',
    'infra/config/rabbitmq/rabbitmq.conf',
    'infra/mysql-init/init.sql',
    'services/docker-compose-services.yml',
    'services/docker-compose-services.prod.yml',
    'services/docker-compose-services.remote.yml',
    'services/.env.example',
    'services/config/api-gateway/api-gateway.toml',
    'observability/docker-compose-observability.yml',
    'observability/docker-compose-observability.staging.yml',
    'observability/docker-compose-observability.prod.yml',
    'observability/docker-compose-observability.remote.yml',
    'observability/.env.example',
    'observability/config/prometheus/prometheus.yml',
    'base-images.json',
    '.env.prod.example',
    'README.md',
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
  assert.ok(rel.includes('infra/docker-compose-infra.yml'), '组子目录内的 compose 必须收录');
  assert.ok(rel.includes('base-images.json'));
  assert.ok(rel.includes('.env.prod.example'), 'whitelisted dotfile kept');
  assert.ok(rel.includes('infra/.env.example'), '组级 .env.example 白名单（自包含部署模板）');
  assert.ok(!rel.includes('README.md'), 'lmms-owned docs excluded — consumers write their own');
  assert.ok(rel.includes(path.join('observability', 'config', 'prometheus', 'prometheus.yml').replaceAll('\\', '/')));
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
  assert.ok(fs.existsSync(path.join(result.dir, 'infra', 'config', 'rabbitmq', 'rabbitmq.conf')));
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

test('pg dialect: infra prod slice gets postgres convergence; include entry untouched', () => {
  const slice = 'services:\n  mysql:\n    ports: !override []\n    restart: unless-stopped\n';
  const out = transformForPg('docker-compose-infra.prod.yml', slice);
  assert.match(out, /postgres:\n {4}ports: !override \[\]\n {4}restart: unless-stopped/);
  assert.ok(out.includes('mysql'), '原 mysql 收敛块保留');
  assert.equal(transformForPg('docker-compose-infra.prod.yml', out), out, '重复变换必须幂等');
  // 三组拆分后入口是纯 include 文件：不做方言变换，原样拷贝。
  const entry = 'include:\n  - docker-compose-infra.prod.yml\n  - docker-compose-services.prod.yml\n';
  assert.equal(transformForPg('docker-compose.prod.yml', entry), null);
});

test('pg dialect: non-transformable files returned as null (copy as-is)', () => {
  assert.equal(transformForPg('base-images.json', '{}'), null);
  assert.equal(transformForPg('observability/config/prometheus/prometheus.yml', 'global: {}\n'), null);
});

test('pg dialect: subdir relpaths dispatch by basename (2026-11-06 自包含子目录布局)', () => {
  // 全 relpath 匹配会对子目录文件静默跳过变换（复制不变换）——必须按 basename 命中。
  const infra = yaml.load(`
services:
  mysql: { image: 'mysql:8.0', profiles: null, healthcheck: {} }
  postgres: { image: 'postgres:16', profiles: ['pg'], healthcheck: {} }
`);
  const out = yaml.load(transformForPg('infra/docker-compose-infra.yml', yaml.dump(infra)));
  assert.equal(out.services.postgres.profiles, undefined, '子目录 infra compose 仍须命中 postgres 提升');
  assert.deepEqual(out.services.mysql.profiles, ['mysql']);

  const svc = yaml.load(`
services:
  token-service:
    environment: { TOKEN_DATABASE_DB_TYPE: mysql }
    depends_on: { mysql: { condition: service_healthy } }
`);
  const out2 = yaml.load(transformForPg('services/docker-compose-services.yml', yaml.dump(svc)));
  assert.equal(out2.services['token-service'].environment.TOKEN_DATABASE_DB_TYPE, 'postgres');
  assert.deepEqual(Object.keys(out2.services['token-service'].depends_on), ['postgres']);

  assert.equal(transformForPg('observability/docker-compose-observability.yml', 'services: {}\n'), null,
    'obs 组 compose 无方言差异，原样拷贝');
  const slice = 'services:\n  mysql:\n    ports: !override []\n';
  assert.match(
    transformForPg('infra/docker-compose-infra.prod.yml', slice),
    /postgres:\n {4}ports: !override \[\]/,
    '子目录内的 infra prod 片仍须追加 postgres 收敛块',
  );
});

test('pg dialect: YAML merge keys are expanded, not round-tripped as literal <<"', () => {
  // js-yaml 往返会把 `<<` 变成带引号的字面量键，docker compose 拒绝渲染
  // （2026-11-06 bundle 实测事故）。变换必须就地展开合并键。
  const text = [
    'x-consul-env: &consul-env',
    '  CONSUL_ADDR: http://consul:8500',
    'services:',
    '  token-service:',
    '    environment:',
    '      <<: *consul-env',
    '      TOKEN_DATABASE_DB_TYPE: mysql',
    '',
  ].join('\n');
  const out = yaml.load(transformForPg('docker-compose-services.yml', text));
  const env = out.services['token-service'].environment;
  assert.equal(env['<<'], undefined, '合并键必须被展开掉');
  assert.equal(env.CONSUL_ADDR, 'http://consul:8500', '锚点内容并入父级');
  assert.equal(env.TOKEN_DATABASE_DB_TYPE, 'postgres', '展开后显式键仍被方言变换命中');
});

test('pg bundle: dir suffix -pg, replica/init dirs excluded, TEMPLATE_INFO.dialect', () => {
  const dir = fixture();
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-out-'));
  const result = buildComposeTemplate({ infraDir: dir, outRoot, version: '9.9.9', dialect: 'pg' });
  assert.ok(result.dir.endsWith('coder-compose-template-9.9.9-pg'));
  assert.ok(!fs.existsSync(path.join(result.dir, 'mysql-replica')), 'replica 目录不进 PG 包');
  assert.ok(!fs.existsSync(path.join(result.dir, 'infra', 'mysql-init')), 'mysql-init 子目录段同样不进 PG 包');
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

test('pg dialect: DB service build blocks get SERVICE_FEATURES=postgres, api-gateway not', () => {
  const doc = yaml.load(`
services:
  config-service:
    build:
      context: ..
      dockerfile: services/config-service/Dockerfile
  api-gateway:
    build:
      context: ..
      dockerfile: services/api-gateway/Dockerfile
`);
  const out = yaml.load(transformForPg('docker-compose-services.yml', yaml.dump(doc)));
  assert.equal(out.services['config-service'].build.args.SERVICE_FEATURES, 'postgres');
  assert.equal(out.services['api-gateway'].build.args, undefined);
});

// ---------------------------------------------------------------------------
// 凭据类文件不得进模板包（2026-11-03 事故：login.json 活令牌被拷进包）
// ---------------------------------------------------------------------------

test('credential files are excluded from the bundle', () => {
  const dir = fixture();
  for (const rel of ['login.json', 'token.json', 'server.pem', 'tls.key', 'secrets/db.json', 'config/creds.json']) {
    const p = path.join(dir, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, '{"token":"secret"}', 'utf-8');
  }
  const outRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tpl-secret-'));
  const result = buildComposeTemplate({ infraDir: dir, outRoot, version: '1.0.0' });
  const walk = (d, acc = []) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const abs = path.join(d, e.name);
      if (e.isDirectory()) walk(abs, acc);
      else acc.push(path.relative(result.dir, abs));
    }
    return acc;
  };
  const files = walk(result.dir);
  for (const f of files) {
    assert.ok(!/login\.json$/.test(f), `login.json 不得进包: ${f}`);
    assert.ok(!/\.pem$|\.key$/.test(f), `密钥文件不得进包: ${f}`);
    assert.ok(!/secrets\/|creds\.json/.test(f), `凭据目录/文件不得进包: ${f}`);
  }
});
