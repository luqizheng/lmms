'use strict';

// Workspace npm package pre-publish checks and tag-versioned publishing.
// Absorbed from the retired check-packages-publish plus the publish_packages
// CI job semantics (npm version from tag, then publish, per package in order).
// @digua/cli joins the set (spec §二.6): pure CJS, no build, published last.

const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { spawnSync } = require('node:child_process');
const { makeExec } = require('./spawn.js');
const { GuardError } = require('./errors.js');

const REGISTRY = 'http://npm.kudi.repos/';

// pnpm/npm are .cmd shims on Windows — spawnSync without a shell can't resolve
// them (same ENOENT the retired scripts hit); Linux CI unaffected.
const SHIM = { shell: process.platform === 'win32' };

const PACKAGES = [
  { dir: 'packages/contracts', name: '@digua/contracts', needsBuild: true },
  { dir: 'packages/http', name: '@digua/http', needsBuild: true },
  { dir: 'packages/cli', name: '@digua/cli', needsBuild: false },
  // 例外（用户裁决 2026-10-31）：npm TS 包不按惯例放 packages/，而放 sdk/iam-services
  { dir: 'sdk/iam-services', name: 'iam-services', needsBuild: true },
];

function normalizeVersion(v) {
  return String(v ?? '').trim().replace(/^v/, '');
}

function pingRegistry(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      res.resume();
      resolve(res.statusCode ?? 0);
    });
    req.on('error', reject);
    req.setTimeout(15000, () => req.destroy(new Error('connection timed out')));
  });
}

async function checkPackages({
  root,
  nextVersion,
  registryUrl = REGISTRY,
  httpGet = pingRegistry,
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, encoding: 'utf-8' }),
  fs: nodeFs = fs,
} = {}) {
  const failures = [];
  const notes = [];
  const check = (label, fn) => {
    try {
      if (fn() === false) failures.push(label);
      else notes.push(`OK  ${label}`);
    } catch (err) {
      failures.push(`${label} — ${err.message}`);
    }
  };

  const version = normalizeVersion(nextVersion);

  for (const pkg of PACKAGES) {
    const pkgJsonPath = path.join(root, pkg.dir, 'package.json');
    let pkgJson = {};
    check(`${pkg.name}: package.json 存在且可解析`, () => {
      pkgJson = JSON.parse(nodeFs.readFileSync(pkgJsonPath, 'utf8'));
      return true;
    });
    check(`${pkg.name}: 名称正确`, () => pkgJson.name === pkg.name);
    check(`${pkg.name}: 未标记 private`, () => pkgJson.private !== true);
    check(`${pkg.name}: publishConfig 指向私有 registry`, () => pkgJson.publishConfig?.registry === registryUrl);

    if (version) {
      check(`${pkg.name}: 当前版本低于目标版本 ${version}`, () => {
        const current = String(pkgJson.version ?? '').split('.').map(Number);
        const next = version.split('.').map(Number);
        if (next.length !== 3 || next.some(Number.isNaN)) throw new Error(`目标版本格式非法：${version}`);
        for (let i = 0; i < 3; i++) {
          if (next[i] > current[i]) return true;
          if (next[i] < current[i]) return false;
        }
        return false;
      });
    }

    if (pkg.needsBuild) {
      check(`${pkg.name}: dist 已构建`, () => {
        if (!nodeFs.existsSync(path.join(root, pkg.dir, 'dist', 'index.js'))) {
          throw new Error('dist/index.js 不存在，先运行 pnpm run packages:build');
        }
        return true;
      });
    }
  }

  check('工作区无未提交的包内改动', () => {
    const res = exec('git', ['status', '--porcelain', '--', ...PACKAGES.map((p) => p.dir)], { cwd: root });
    const diff = String(res.stdout ?? '').trim();
    if (diff) throw new Error(`存在未提交改动：\n${diff}`);
    return true;
  });

  try {
    const code = await httpGet(registryUrl);
    const s = String(code);
    if (s.startsWith('2') || s.startsWith('3')) notes.push(`OK  私有 registry 可达（${registryUrl}） —— HTTP ${s}`);
    else failures.push(`私有 registry 可达（${registryUrl}） —— HTTP ${s}`);
  } catch (err) {
    failures.push(`私有 registry 可达（${registryUrl}） —— ${err.message}`);
  }

  return { ok: failures.length === 0, failures, notes };
}

async function publishPackages({
  root,
  version,
  dryRun = false,
  json = false,
  exec = makeExec({ json }),
  check = checkPackages,
  registryUrl = REGISTRY,
  out = { log: (m) => console.log(m), warn: (m) => console.warn(m) },
} = {}) {
  const v = normalizeVersion(version);
  if (!v) throw new GuardError('NPM_VERSION_REQUIRED', '--version <x.y.z> is required');

  out.log('[npm-packages] building workspace packages (pnpm run packages:build)');
  const build = exec('pnpm', ['run', 'packages:build'], { cwd: root, ...SHIM });
  if ((build.status ?? 1) !== 0) throw new GuardError('NPM_BUILD_FAILED', 'pnpm run packages:build failed');

  const result = await check({ root, nextVersion: v, registryUrl });
  if (!result.ok) {
    throw new GuardError('NPM_PACKAGES_CHECK_FAILED', `pre-publish check failed:\n  ${result.failures.join('\n  ')}`, { failures: result.failures });
  }

  if (dryRun) {
    const planned = PACKAGES.flatMap((p) => [
      `npm --prefix ${p.dir} version ${v} --no-git-tag-version --allow-same-version`,
      `npm publish --access public  (cwd=${p.dir})`,
    ]);
    return { published: [], planned, dryRun: true };
  }

  const published = [];
  for (const pkg of PACKAGES) {
    const ver = exec('npm', ['--prefix', pkg.dir, 'version', v, '--no-git-tag-version', '--allow-same-version'], { cwd: root, ...SHIM });
    if ((ver.status ?? 1) !== 0) throw new GuardError('NPM_VERSION_FAILED', `npm version failed for ${pkg.name}`);
    // npm>=11 的 `publish --prefix` 会从 cwd（仓库根）读 manifest，version 为 null 直接崩；
    // 改为在包目录内执行，manifest/publishConfig 均取包自身。
    // 另：@scope:registry 配置（如 ~/.npmrc）优先级高于包内 publishConfig.registry，
    // 这里显式钉死发布目标，保证与守卫校验的 REGISTRY 一致。
    const publishArgs = ['publish', '--access', 'public', `--registry=${registryUrl}`];
    const scope = /^(@[^/]+)\//.exec(pkg.name);
    if (scope) publishArgs.push(`--${scope[1]}:registry=${registryUrl}`);
    const pub = exec('npm', publishArgs, { cwd: path.join(root, pkg.dir), ...SHIM });
    if ((pub.status ?? 1) !== 0) throw new GuardError('NPM_PUBLISH_FAILED', `npm publish failed for ${pkg.name}`);
    published.push({ name: pkg.name, version: v });
  }
  return { published };
}

module.exports = { PACKAGES, REGISTRY, checkPackages, publishPackages };
