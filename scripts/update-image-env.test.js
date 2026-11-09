'use strict';

// scripts/update-image-env.test.js —— .image.env 生成器与 compose 变量的一致性校验
//
// 守两条不变量：
//   1. compose 的 image: 引用的 ${IMAGE_*} 变量集合 == .image.env 里声明的变量集合
//      （少一个 → 部署时 compose 硬报错；多一个 → 残留变量无人使用）
//   2. 入库的 .image.env == 脚本按其自身版本/方言重新生成的内容（幂等、可复现）

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const {
  SERVICES,
  DIALECT_SUFFIX,
  normalizeVersion,
  validateVersion,
  buildImageEnv,
  parseArgs,
  deriveVersionDialect,
} = require('./update-image-env');

const REPO_ROOT = path.resolve(__dirname, '..');
const DEPLOY_DIR = path.join(REPO_ROOT, 'examples', 'docker-deploy');
const IMAGE_ENV = path.join(DEPLOY_DIR, '.image.env');
const COMPOSE = path.join(DEPLOY_DIR, 'docker-compose-services.yml');

function readImageEnv() {
  const raw = fs.readFileSync(IMAGE_ENV, 'utf8');
  const vars = new Map();
  for (const line of raw.split('\n')) {
    const m = line.match(/^(IMAGE_[A-Z_]+)=(.*)$/);
    if (m) vars.set(m[1], m[2]);
  }
  return { raw, vars };
}

function composeImageVars() {
  const raw = fs.readFileSync(COMPOSE, 'utf8');
  const vars = new Set();
  for (const m of raw.matchAll(/image:\s*"\$\{(IMAGE_[A-Z_]+):\?/g)) vars.add(m[1]);
  return vars;
}

test('normalizeVersion 剥掉前导 v（CI 推送用 ${CI_COMMIT_TAG#v}）', () => {
  assert.strictEqual(normalizeVersion('v1.7.0'), '1.7.0');
  assert.strictEqual(normalizeVersion('1.7.0'), '1.7.0');
  assert.strictEqual(normalizeVersion('v1.7.0-pg'), '1.7.0-pg');
});

test('validateVersion 拒绝非法版本号', () => {
  assert.ok(validateVersion('1.7.0'));
  assert.ok(validateVersion('1.7.0-pg'));
  assert.ok(!validateVersion(''));
  assert.ok(!validateVersion('-1.7.0'));
  assert.ok(!validateVersion('1.7.0;rm -rf'));
});

test('pg 方言：带 DB 的服务加 -pg 后缀，网关不加', () => {
  const out = buildImageEnv({ version: '1.8.0', dialect: 'pg' });
  assert.ok(out.includes('IMAGE_API_GATEWAY=zhcoder-docker-registry.com:8000/micro-server/api-gateway:1.8.0\n'));
  assert.ok(out.includes('IMAGE_TOKEN_SERVICE=zhcoder-docker-registry.com:8000/micro-server/token-service:1.8.0-pg\n'));
  assert.ok(out.includes('IMAGE_PRUE_BACKEND=zhcoder-docker-registry.com:8000/micro-server/prue-admin-backend:1.8.0-pg\n'));
  assert.ok(!/IMAGE_API_GATEWAY=.*-pg/.test(out));
});

test('mysql 方言：无后缀', () => {
  const out = buildImageEnv({ version: '1.8.0', dialect: 'mysql' });
  assert.ok(out.includes('IMAGE_TOKEN_SERVICE=zhcoder-docker-registry.com:8000/micro-server/token-service:1.8.0\n'));
  const assignments = out.split('\n').filter((l) => l.startsWith('IMAGE_'));
  assert.ok(assignments.length > 0);
  for (const line of assignments) assert.ok(!line.endsWith('-pg'), `mysql 方言不该有 -pg 后缀: ${line}`);
});

test('非法方言 / 非法版本号抛错', () => {
  assert.throws(() => buildImageEnv({ version: '1.8.0', dialect: 'oracle' }), /invalid dialect/);
  assert.throws(() => buildImageEnv({ version: 'x y' }), /invalid version/);
});

test('生成的变量集合与 SERVICES 一一对应（无重名）', () => {
  const envs = SERVICES.map((s) => s.env);
  assert.strictEqual(new Set(envs).size, envs.length);
  const out = buildImageEnv({ version: '1.0.0' });
  for (const e of envs) assert.ok(out.includes(`${e}=`), `缺少变量 ${e}`);
});

test('方言后缀表：pg → -pg，mysql → 空', () => {
  assert.strictEqual(DIALECT_SUFFIX.pg, '-pg');
  assert.strictEqual(DIALECT_SUFFIX.mysql, '');
});

test('parseArgs 解析版本/方言/目录与开关', () => {
  const a = parseArgs(['-v', '1.8.0', '--dialect', 'mysql', '--dir', 'x/y', '--check', '--verify']);
  assert.strictEqual(a.version, '1.8.0');
  assert.strictEqual(a.dialect, 'mysql');
  assert.strictEqual(a.dir, 'x/y');
  assert.strictEqual(a.check, true);
  assert.strictEqual(a.verify, true);
});

test('compose 的 image: 变量集合 == .image.env 声明的变量集合', () => {
  const { vars } = readImageEnv();
  const used = composeImageVars();
  assert.deepStrictEqual(
    [...used].sort(),
    [...vars.keys()].sort(),
    'compose 引用的 IMAGE_* 与 .image.env 声明的不一致'
  );
});

test('入库的 .image.env == 脚本按自身版本/方言重新生成的内容（幂等）', () => {
  const { raw } = readImageEnv();
  const derived = deriveVersionDialect(IMAGE_ENV);
  assert.ok(derived, '无法从 .image.env 反推版本/方言');

  const regenerated = buildImageEnv({ version: derived.version, dialect: derived.dialect });
  assert.strictEqual(
    raw,
    regenerated,
    `.image.env 与脚本生成结果不一致（version=${derived.version}, dialect=${derived.dialect}）——跑 node scripts/update-image-env.js -v ${derived.version} --dialect ${derived.dialect}`
  );
});

test('.image.env 每条镜像引用都是 <registry>/<repo>/<image>:<tag> 形态（无 :latest）', () => {
  const { vars } = readImageEnv();
  for (const [k, v] of vars) {
    assert.match(v, /^[^/\s:]+(?::\d+)?\/[^/\s]+\/[^:\s]+:[^:\s]+$/, `${k} 形态非法: ${v}`);
    assert.ok(!v.endsWith(':latest'), `${k} 不得指向 :latest`);
  }
});
