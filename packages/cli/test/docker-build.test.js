'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { SERVICES, DEFAULT_REGISTRY, parseBuildArgs, resolvePlan, buildService, buildImages, pruneBuildCache } = require('../lib/docker-build.js');

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-dockerbuild-'));
  const df = path.join(dir, 'services', 'api-gateway', 'Dockerfile');
  fs.mkdirSync(path.dirname(df), { recursive: true });
  fs.writeFileSync(df, 'FROM scratch\n');
  return dir;
}

const quietOut = { log() {}, warn() {}, error() {} };

test('parseBuildArgs reads services, version and push flag', () => {
  const a = parseBuildArgs(['api-gateway', 'token-service', '-v', '1.6.0', '--push']);
  assert.deepEqual(a.services, ['api-gateway', 'token-service']);
  assert.equal(a.version, '1.6.0');
  assert.equal(a.push, true);
  assert.equal(a.loadAndPush, false);
});

test('unknown positional names and flags are collected, not fatal', () => {
  const a = parseBuildArgs(['bogus', 'api-gateway', '--wat']);
  assert.deepEqual(a.services, ['api-gateway']);
  assert.ok(a.unknown.includes('bogus') || a.unknown.includes('--wat'));
});

test('prune keyword switches to cache-clean mode', () => {
  assert.equal(parseBuildArgs(['prune']).prune, true);
  assert.equal(parseBuildArgs(['clean']).prune, true);
  assert.equal(parseBuildArgs(['api-gateway']).prune, false);
});

test('resolvePlan non-TTY without flags falls back to local full build', () => {
  const plan = resolvePlan({ args: parseBuildArgs([]), isTTY: false });
  assert.equal(plan.mode, 'local');
  assert.deepEqual(plan.services, SERVICES);
  assert.equal(plan.version, null);
});

test('resolvePlan with explicit version and --push', () => {
  const plan = resolvePlan({ args: parseBuildArgs(['api-gateway', '-v', '1.6.0', '--push']), isTTY: true });
  assert.equal(plan.mode, 'push');
  assert.deepEqual(plan.services, ['api-gateway']);
  assert.equal(plan.version, '1.6.0');
});

test('resolvePlan multi-arch load-and-push falls back to push with warnings', () => {
  const plan = resolvePlan({ args: parseBuildArgs(['--load-and-push', '-p', 'linux/amd64,linux/arm64']), isTTY: true });
  assert.equal(plan.mode, 'push');
  assert.ok(plan.warnings.length > 0);
});

test('buildImages local mode: one buildx build, versioned + latest tags, never push', () => {
  const dir = fixture();
  const calls = [];
  const exec = (file, args) => { calls.push([file, ...args]); return { status: 0 }; };
  const plan = resolvePlan({ args: parseBuildArgs(['api-gateway', '-v', '1.6.0']), isTTY: false });
  const res = buildImages({ root: dir, plan, exec, out: quietOut });

  assert.deepEqual(res.built.map((b) => b.service), ['api-gateway']);
  assert.deepEqual(res.failed, []);
  const build = calls.find((c) => c[0] === 'docker' && c[1] === 'buildx');
  assert.ok(build, 'docker buildx build invoked');
  const tags = build.filter((x, i) => build[i - 1] === '-t');
  assert.deepEqual(tags, [
    `${DEFAULT_REGISTRY}/micro-server/api-gateway:1.6.0`,
    `${DEFAULT_REGISTRY}/micro-server/api-gateway:latest`,
  ]);
  assert.ok(build.includes('--load'), 'local mode uses --load');
  assert.ok(!calls.some((c) => c.includes('push')), 'local mode never pushes');
});

test('buildImages load-and-push pushes each tag after a successful load', () => {
  const dir = fixture();
  const calls = [];
  const exec = (file, args) => { calls.push([file, ...args]); return { status: 0 }; };
  const plan = resolvePlan({ args: parseBuildArgs(['api-gateway', '-v', '1.6.0', '--load-and-push']), isTTY: true });
  buildImages({ root: dir, plan, exec, out: quietOut });

  const pushes = calls.filter((c) => c[0] === 'docker' && c[1] === 'push');
  assert.equal(pushes.length, 2, 'one docker push per tag');
  assert.ok(pushes[0].includes(`${DEFAULT_REGISTRY}/micro-server/api-gateway:1.6.0`));
});

test('buildImages collects failures without stopping the rest', () => {
  const dir = fixture();
  const calls = [];
  const exec = (file, args) => {
    calls.push([file, ...args]);
    const isPrue = args.includes('services/prue_backend/Dockerfile');
    return { status: isPrue ? 3 : 0 };
  };
  const plan = resolvePlan({ args: parseBuildArgs(['api-gateway', 'prue-admin-backend', '-v', '1.6.0']), isTTY: false });
  const res = buildImages({ root: dir, plan, exec, out: quietOut });
  assert.deepEqual(res.built.map((b) => b.service), ['api-gateway']);
  assert.deepEqual(res.failed.map((f) => f.service), ['prue-admin-backend']);
});

test('pruneBuildCache targets cargo cache filters only', () => {
  const calls = [];
  const exec = (file, args) => { calls.push([file, ...args]); return { status: 0 }; };
  pruneBuildCache({ exec, out: quietOut });
  assert.equal(calls.length, 2);
  for (const c of calls) {
    assert.deepEqual(c.slice(0, 3), ['docker', 'buildx', 'prune']);
    assert.ok(c.includes('--cache-filter'));
    assert.ok(!c.includes('-af'), 'never nukes the whole buildx cache');
  }
});

test('pg dialect: DB services only, -pg tags, postgres feature build-arg', () => {
  const plan = resolvePlan({ args: parseBuildArgs(['--dialect', 'pg', '-v', '1.6.0']), isTTY: false });
  assert.equal(plan.dialect, 'pg');
  assert.deepEqual(
    plan.services.sort(),
    ['audit-service', 'file-service', 'iam-service', 'notification-service', 'prue-admin-backend', 'token-service'],
  );
  assert.deepEqual(plan.tagsByService['token-service'], [
    `${DEFAULT_REGISTRY}/micro-server/token-service:1.6.0-pg`,
    `${DEFAULT_REGISTRY}/micro-server/token-service:pg-latest`,
  ]);
  assert.ok(plan.warnings.some((w) => w.includes('api-gateway')), '非 DB 服务必须提示跳过');
});

test('pg dialect: buildService passes SERVICE_FEATURES=postgres build-arg', () => {
  const calls = [];
  const exec = (file, args) => { calls.push([file, ...args]); return { status: 0 }; };
  const root = fixture();
  fs.mkdirSync(path.join(root, 'services', 'token-service'), { recursive: true });
  fs.writeFileSync(path.join(root, 'services', 'token-service', 'Dockerfile'), 'FROM scratch\n');
  const plan = resolvePlan({ args: parseBuildArgs(['--dialect', 'pg', '-v', '1.6.0', 'token-service']), isTTY: false });
  buildService('token-service', plan, { root, exec, out: { log: () => {}, warn: () => {} } });
  const build = calls.find((c) => c[0] === 'docker');
  const i = build.indexOf('--build-arg');
  assert.ok(i > 0, 'pg 构建必须带 --build-arg');
  assert.deepEqual(build.slice(i, i + 2), ['--build-arg', 'SERVICE_FEATURES=postgres']);
});

test('mysql dialect (default): tags unchanged and no build-arg', () => {
  const plan = resolvePlan({ args: parseBuildArgs(['-v', '1.6.0']), isTTY: false });
  assert.equal(plan.dialect, 'mysql');
  assert.deepEqual(plan.tagsByService['token-service'], [
    `${DEFAULT_REGISTRY}/micro-server/token-service:1.6.0`,
    `${DEFAULT_REGISTRY}/micro-server/token-service:latest`,
  ]);
  assert.ok(!plan.warnings.some((w) => w.includes('skipping non-DB')));
});

test('invalid dialect is rejected', () => {
  assert.throws(() => resolvePlan({ args: parseBuildArgs(['--dialect', 'oracle']), isTTY: false }), /invalid dialect/);
});
