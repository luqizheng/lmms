'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const SCRIPT = path.join(__dirname, 'transform-compose.js');

const COMPOSE = `services:
  config-service:
    build:
      context: ..
      dockerfile: services/config-service/Dockerfile
    environment:
      DATABASE_URL: "\${DATABASE_URL}"
  iam-outbox-publisher:
    build:
      context: ..
      dockerfile: services/iam-service/Dockerfile
    command: ["iam_outbox_publisher"]
  unknown-thing:
    build:
      context: ..
      dockerfile: services/other/Dockerfile
`;

function run(args) {
  return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
}

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-tc-'));
  const file = path.join(dir, 'docker-compose-services.yml');
  fs.writeFileSync(file, COMPOSE, 'utf-8');
  return file;
}

test('mysql dialect (default): tag has no suffix', () => {
  const file = fixture();
  const res = run([file, '1.6.0']);
  assert.equal(res.status, 0, res.stderr);
  const out = fs.readFileSync(file, 'utf-8');
  assert.match(out, /config-service:\$\{IMAGE_TAG:-1\.6\.0\}/);
  assert.ok(!out.includes('1.6.0-pg'), 'mysql 变体不得带 -pg 后缀');
});

test('pg dialect: tag carries -pg suffix for every mapped service', () => {
  const file = fixture();
  const res = run([file, '1.6.0', '--dialect', 'pg']);
  assert.equal(res.status, 0, res.stderr);
  const out = fs.readFileSync(file, 'utf-8');
  assert.match(out, /config-service:\$\{IMAGE_TAG:-1\.6\.0-pg\}/);
  // outbox publisher 复用主服务镜像 → 同样带 -pg
  assert.match(out, /iam-service:\$\{IMAGE_TAG:-1\.6\.0-pg\}/);
  // 未在 IMAGE_MAP 里的服务保持 build: 块不动
  assert.match(out, /unknown-thing:\n {4}build:/);
});

test('pg dialect via positional arg (legacy-registry placeholder)', () => {
  const file = fixture();
  const res = run([file, '1.6.0', 'x', 'pg']);
  assert.equal(res.status, 0, res.stderr);
  assert.match(fs.readFileSync(file, 'utf-8'), /IMAGE_TAG:-1\.6\.0-pg/);
});

test('unknown dialect is rejected with exit 1', () => {
  const file = fixture();
  const res = run([file, '1.6.0', '--dialect', 'oracle']);
  assert.equal(res.status, 1);
  assert.match(res.stderr, /未知方言/);
});

test('v prefix is stripped from the version', () => {
  const file = fixture();
  const res = run([file, 'v1.6.0', '--dialect', 'pg']);
  assert.equal(res.status, 0, res.stderr);
  assert.match(fs.readFileSync(file, 'utf-8'), /IMAGE_TAG:-1\.6\.0-pg/);
});
