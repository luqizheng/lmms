#!/usr/bin/env node
/**
 * image-resolver.test.js — 共享镜像解析模块的单测
 *
 * 覆盖：
 *   - ${VAR:-default} 默认值解析
 *   - ${VAR}/path 拼接型解析
 *   - 进程环境变量覆盖 :- 默认值
 *   - env 文件优先于 :- 默认值
 *   - 既无环境值也无默认值 → unresolved（不得静默跳过）
 *   - tag 归一（去 v 前缀）
 */

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  scanComposeImages,
  interpolate,
  normalizeImageTag,
  parseEnvFile,
} = require('./image-resolver.js');

// ── 辅助：建临时 compose 目录 ───────────────────────────────────────────────
function tmpCompose(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'img-resolver-'));
  for (const [name, content] of Object.entries(files)) {
    fs.writeFileSync(path.join(dir, name), content, 'utf-8');
  }
  return dir;
}

// ── interpolate ──────────────────────────────────────────────────────────────

test('interpolate: ${VAR:-default} 取默认值', () => {
  const r = interpolate('${redis:-redis:7-alpine}', () => undefined);
  assert.strictEqual(r.value, 'redis:7-alpine');
  assert.deepStrictEqual(r.unresolved, []);
});

test('interpolate: 已定义值优先于默认值', () => {
  const r = interpolate('${redis:-redis:7-alpine}', () => 'other/redis:7-alpine');
  assert.strictEqual(r.value, 'other/redis:7-alpine');
});

test('interpolate: 拼接型默认值', () => {
  const r = interpolate('${kudi_repos:-reg/micro-server}/api:${IMAGE_TAG:-1.6.0}', () => undefined);
  assert.strictEqual(r.value, 'reg/micro-server/api:1.6.0');
});

test('interpolate: 无默认且未定义 → 记入 unresolved', () => {
  const r = interpolate('${kudi_repos}/api:1.6.0', () => undefined);
  assert.strictEqual(r.value, '/api:1.6.0');
  assert.deepStrictEqual(r.unresolved, ['kudi_repos']);
});

test('interpolate: 多变量混合', () => {
  const vars = { a: 'A', b: undefined };
  const r = interpolate('${a}-${b:-B}-${c:-C}', (n) => vars[n]);
  assert.strictEqual(r.value, 'A-B-C');
  assert.deepStrictEqual(r.unresolved, []);
});

test('interpolate: 空字符串视为未定义（走默认值）', () => {
  const r = interpolate('${redis:-redis:7-alpine}', () => '');
  assert.strictEqual(r.value, 'redis:7-alpine');
});

// ── normalizeImageTag ───────────────────────────────────────────────────────

test('normalizeImageTag: 剥掉 v 前缀', () => {
  assert.strictEqual(normalizeImageTag('v1.5.0'), '1.5.0');
  assert.strictEqual(normalizeImageTag('1.5.0'), '1.5.0');
});

test('normalizeImageTag: 不误伤非数字 v', () => {
  assert.strictEqual(normalizeImageTag('vnext'), 'vnext');
  assert.strictEqual(normalizeImageTag('abc123'), 'abc123');
});

test('normalizeImageTag: SHA 类 tag 保持原样', () => {
  assert.strictEqual(normalizeImageTag('7cde9007'), '7cde9007');
});

// ── parseEnvFile ────────────────────────────────────────────────────────────

test('parseEnvFile: 解析键值对、忽略注释', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'img-env-'));
  const f = path.join(dir, '.env');
  fs.writeFileSync(f, [
    '# 注释',
    '',
    'REDIS=redis:7-alpine',
    'export EXPORTED=exported-value',
    'QUOTED="quoted value"',
    'WITH_HASH=val # trailing comment',
  ].join('\n'), 'utf-8');
  const vars = parseEnvFile(f);
  assert.strictEqual(vars.REDIS, 'redis:7-alpine');
  assert.strictEqual(vars.EXPORTED, 'exported-value');
  assert.strictEqual(vars.QUOTED, 'quoted value');
  assert.strictEqual(vars.WITH_HASH, 'val');
});

test('parseEnvFile: 文件不存在返回空对象', () => {
  assert.deepStrictEqual(parseEnvFile('/nonexistent/.env'), {});
});

// ── scanComposeImages ───────────────────────────────────────────────────────

test('scanComposeImages: 解析多个 compose 的 image: 行', () => {
  const dir = tmpCompose({
    'docker-compose.yml': [
      'services:',
      '  redis:',
      '    image: ${redis:-redis:7-alpine}',
      '  mysql:',
      '    image: ${mysql:-mysql:8.0}',
    ].join('\n'),
    'docker-compose-infra.yml': [
      'services:',
      '  consul:',
      '    image: ${consul:-hashicorp/consul:2.0.1}',
    ].join('\n'),
  });
  const r = scanComposeImages(dir, { env: {} });
  assert.deepStrictEqual(r.images, [
    'hashicorp/consul:2.0.1',
    'mysql:8.0',
    'redis:7-alpine',
  ]);
  assert.deepStrictEqual(r.unresolved, []);
});

test('scanComposeImages: 平台镜像拼接型展开', () => {
  const dir = tmpCompose({
    'docker-compose.yml': [
      'services:',
      '  api-gateway:',
      '    image: ${kudi_repos:-reg/micro-server}/api-gateway:${IMAGE_TAG:-1.6.0}',
    ].join('\n'),
  });
  const r = scanComposeImages(dir, { env: {} });
  assert.deepStrictEqual(r.images, ['reg/micro-server/api-gateway:1.6.0']);
  assert.deepStrictEqual(r.unresolved, []);
});

test('scanComposeImages: 进程环境变量覆盖 compose 默认值', () => {
  const dir = tmpCompose({
    'docker-compose.yml': 'services:\n  redis:\n    image: ${redis:-redis:7-alpine}',
  });
  const r = scanComposeImages(dir, { env: { redis: 'docker.1ms.run/redis:7-alpine' } });
  assert.deepStrictEqual(r.images, ['docker.1ms.run/redis:7-alpine']);
});

test('scanComposeImages: env 文件优先于 compose 默认值', () => {
  const dir = tmpCompose({
    'docker-compose.yml': 'services:\n  redis:\n    image: ${redis:-redis:7-alpine}',
  });
  const envPath = path.join(dir, '.env');
  fs.writeFileSync(envPath, 'redis=mirror.internal/redis:7-alpine\n', 'utf-8');
  const r = scanComposeImages(dir, { envFile: envPath, env: {} });
  assert.deepStrictEqual(r.images, ['mirror.internal/redis:7-alpine']);
});

test('scanComposeImages: 进程环境变量优先于 env 文件', () => {
  const dir = tmpCompose({
    'docker-compose.yml': 'services:\n  redis:\n    image: ${redis:-redis:7-alpine}',
  });
  const envPath = path.join(dir, '.env');
  fs.writeFileSync(envPath, 'redis=from-file/redis:7-alpine\n', 'utf-8');
  const r = scanComposeImages(dir, { envFile: envPath, env: { redis: 'from-env/redis:7-alpine' } });
  assert.deepStrictEqual(r.images, ['from-env/redis:7-alpine']);
});

test('scanComposeImages: 无默认值且未定义 → unresolved 非空（不得静默跳过）', () => {
  const dir = tmpCompose({
    'docker-compose.yml': 'services:\n  gw:\n    image: ${kudi_repos}/api-gateway:1.6.0',
  });
  const r = scanComposeImages(dir, { env: {} });
  assert.deepStrictEqual(r.unresolved, ['kudi_repos']);
});

test('scanComposeImages: 忽略注释行与空 image', () => {
  const dir = tmpCompose({
    'docker-compose.yml': [
      'services:',
      '  a:',
      '    image: # commented out',
      '  b:',
      '    image: ""',
    ].join('\n'),
  });
  const r = scanComposeImages(dir, { env: {} });
  assert.deepStrictEqual(r.images, []);
});

test('scanComposeImages: 只扫指定文件（files 参数）', () => {
  const dir = tmpCompose({
    'docker-compose.yml': 'services:\n  a:\n    image: ${a:-a:1}',
    'other.yml': 'services:\n  b:\n    image: ${b:-b:1}',
  });
  const r = scanComposeImages(dir, { files: ['docker-compose.yml'], env: {} });
  assert.deepStrictEqual(r.images, ['a:1']);
});

test('scanComposeImages: 目录不存在返回空结果', () => {
  const r = scanComposeImages('/nonexistent/dir', { env: {} });
  assert.deepStrictEqual(r.images, []);
  assert.deepStrictEqual(r.unresolved, []);
});
