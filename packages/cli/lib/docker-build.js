'use strict';

// Service image builds via buildx. Absorbed from the retired build-docker script with
// identical semantics: versioned + latest tags, multi-arch forces --push,
// load-and-push falls back to --push on multi-arch, docs-site gets a host-side
// vitepress pre-build, prune only clears the cargo cache filters.

const fs = require('node:fs');
const path = require('node:path');
const { makeExec } = require('./spawn.js');

const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000';
const DEFAULT_REPO = 'micro-server';
const DEFAULT_PLATFORM = 'linux/amd64';

const SERVICES = [
  'api-gateway',
  'token-service',
  'iam-service',
  'notification-service',
  'file-service',
  'config-service',
  'audit-service',
  'prue-admin-backend',
  'docs-site',
];

// 带 DB 的服务（Dockerfile 声明了 SERVICE_FEATURES ARG）。非 DB 服务
// （api-gateway / docs-site）镜像与方言无关，PG 构建时跳过并提示。
const DB_SERVICES = new Set([
  'token-service',
  'iam-service',
  'notification-service',
  'file-service',
  'config-service',
  'audit-service',
  'prue-admin-backend',
]);

// 方言 → Cargo feature 集合与 tag 后缀。mysql 是历史默认（tag 无后缀）；
// pg 变体 tag 加 -pg 后缀（如 :1.6.0-pg / :pg-latest），与 mysql 版本并存。
const DIALECTS = {
  mysql: { features: 'mysql', tagSuffix: '' },
  pg: { features: 'postgres', tagSuffix: '-pg' },
};

function validateVersion(v) {
  if (!v) return true;
  return /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(v);
}

function parsePlatforms(s) {
  if (!s) return null;
  const list = String(s).split(',').map((x) => x.trim()).filter(Boolean);
  if (list.length === 0) return null;
  for (const p of list) {
    if (!/^linux\//.test(p)) throw new Error(`invalid platform "${p}", expected linux/<arch>`);
  }
  return list;
}

function parseBuildArgs(argv) {
  const args = {
    services: [],
    version: null,
    push: false,
    loadAndPush: false,
    platform: null,
    registry: DEFAULT_REGISTRY,
    repo: DEFAULT_REPO,
    dialect: 'mysql',
    prune: false,
    help: false,
    unknown: [],
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '-h': case '--help': args.help = true; break;
      case '-v': case '--version': args.version = argv[++i] || null; break;
      case '--push': args.push = true; args.loadAndPush = false; break;
      case '--no-push': args.push = false; args.loadAndPush = false; break;
      case '--load-and-push': case '--load-push': args.loadAndPush = true; args.push = false; break;
      case '-p': case '--platform': args.platform = argv[++i] || null; break;
      case '--registry': args.registry = argv[++i] || DEFAULT_REGISTRY; break;
      case '--repo': args.repo = argv[++i] || DEFAULT_REPO; break;
      case '--dialect': {
        const d = String(argv[++i] || '').toLowerCase();
        args.dialect = d === 'postgres' ? 'pg' : d;
        break;
      }
      case 'prune': case 'clean': args.prune = true; break;
      default:
        if (SERVICES.includes(a)) args.services.push(a);
        else args.unknown.push(a);
    }
  }
  return args;
}

function emptyArgs(args) {
  return args.services.length === 0 && args.version == null && !args.push && !args.loadAndPush && args.platform == null;
}

// Pure planner. mode: 'push' | 'load-and-push' | 'local'. Multi-arch always
// ends up 'push' (buildx multi-arch cannot --load); falls back with warnings.
function resolvePlan({ args, isTTY }) {
  if (emptyArgs(args) && isTTY) {
    return { interactive: true, warnings: [] };
  }
  const warnings = [];
  if (args.unknown.length > 0) warnings.push(`ignored unknown args: ${args.unknown.join(', ')}`);

  let platforms = args.platform ? parsePlatforms(args.platform) : null;
  if (!platforms) platforms = [DEFAULT_PLATFORM];
  const multiArch = platforms.length > 1;

  let mode = args.loadAndPush ? 'load-and-push' : args.push ? 'push' : 'local';
  if (multiArch) {
    if (mode === 'local') warnings.push('multi-arch cannot --load; switched to --push');
    if (mode === 'load-and-push') warnings.push('multi-arch: --load-and-push falls back to --push');
    mode = 'push';
  }

  if (args.version != null && !validateVersion(args.version)) {
    throw new Error(`invalid version: ${args.version} (allowed: alphanumerics . _ -)`);
  }
  const dialect = DIALECTS[args.dialect] ? args.dialect : null;
  if (!dialect) {
    throw new Error(`invalid dialect: ${args.dialect} (allowed: mysql | pg)`);
  }

  // PG 变体只构建带 DB 的服务；非 DB 服务与方言无关，跳过并提示。
  let services = args.services.length > 0 ? args.services.slice() : SERVICES.slice();
  if (dialect === 'pg') {
    const skipped = services.filter((s) => !DB_SERVICES.has(s));
    if (skipped.length > 0) {
      warnings.push(`pg dialect: skipping non-DB services (dialect-agnostic): ${skipped.join(', ')}`);
    }
    services = services.filter((s) => DB_SERVICES.has(s));
  }

  const registry = args.registry || DEFAULT_REGISTRY;
  const repo = args.repo || DEFAULT_REPO;
  const suffix = DIALECTS[dialect].tagSuffix;
  const tagsByService = Object.fromEntries(services.map((svc) => {
    const base = `${registry}/${repo}/${svc}`;
    const tags = args.version
      ? [`${base}:${args.version}${suffix}`, `${base}:${suffix ? 'pg-' : ''}latest`]
      : [`${base}:${suffix ? 'pg-' : ''}latest`];
    return [svc, [...new Set(tags)]];
  }));

  return { services, version: args.version, platforms, mode, registry, repo, dialect, tagsByService, warnings, interactive: false };
}

function dockerfileFor(root, name) {
  if (name === 'docs-site') {
    // vitepress deps live in the root package.json; context must be the root
    return { file: 'docs-site/Dockerfile', cwd: root };
  }
  if (name === 'prue-admin-backend') {
    // image name prue-admin-backend, source dir prue_backend
    return { file: 'services/prue_backend/Dockerfile', cwd: root };
  }
  return { file: `services/${name}/Dockerfile`, cwd: root };
}

function buildService(name, plan, { root, exec, out }) {
  const { file: relDf, cwd } = dockerfileFor(root, name);
  if (!fs.existsSync(path.join(root, relDf))) {
    out.warn(`[skip] ${name}: no Dockerfile at ${relDf}`);
    return { ok: false, reason: 'no Dockerfile' };
  }

  if (name === 'docs-site') {
    // the image only packs pre-built output; vitepress needs a full git repo
    out.log(`[pre-build] pnpm docs:build`);
    const pre = exec('pnpm', ['docs:build'], { cwd: root, shell: process.platform === 'win32' });
    if (pre.error || (pre.status ?? 1) !== 0) {
      out.warn(`[pre-build-fail] docs-site: pnpm docs:build failed`);
      return { ok: false, reason: 'pre-build failed' };
    }
  }

  const tags = plan.tagsByService[name];
  const buildCmd = ['buildx', 'build'];
  buildCmd.push(plan.mode === 'push' ? '--push' : '--load');
  buildCmd.push('--platform', plan.platforms.join(','));
  buildCmd.push('-f', relDf);
  if (plan.dialect === 'pg') {
    buildCmd.push('--build-arg', 'SERVICE_FEATURES=postgres');
  }
  for (const t of tags) buildCmd.push('-t', t);
  buildCmd.push('.');

  out.log(`[build] ${name} -> ${tags.join(', ')}`);
  const res = exec('docker', buildCmd, { cwd });
  if (res.error || (res.status ?? 1) !== 0) {
    return { ok: false, reason: res.error ? res.error.message : `buildx exit ${res.status}` };
  }

  if (plan.mode === 'load-and-push') {
    for (const t of tags) {
      out.log(`[push] ${t}`);
      const r = exec('docker', ['push', t], { cwd: root });
      if (r.error || (r.status ?? 1) !== 0) {
        return { ok: false, reason: `push failed: ${t}` };
      }
    }
  }
  return { ok: true, tags };
}

// exec default mirrors the retired script: spawnSync with inherited stdio.
function buildImages({ root, plan, json = false, exec = makeExec({ json }), out }) {
  const built = [];
  const failed = [];
  for (const name of plan.services) {
    const result = buildService(name, plan, { root, exec, out });
    if (result.ok) built.push({ service: name, tags: result.tags });
    else failed.push({ service: name, reason: result.reason });
  }
  return { built, failed };
}

function pruneBuildCache({ json = false, exec = makeExec({ json }), out }) {
  const pruned = [];
  const failed = [];
  for (const id of ['cargo-registry', 'cargo-git']) {
    out.log(`[clean] buildx prune cache filter ${id}`);
    const res = exec('docker', ['buildx', 'prune', '--force', '--cache-filter', `id==${id}`], { cwd: process.cwd() });
    if (res.error || (res.status ?? 1) !== 0) failed.push(id);
    else pruned.push(id);
  }
  return { pruned, failed };
}

module.exports = {
  SERVICES,
  DB_SERVICES,
  DIALECTS,
  DEFAULT_REGISTRY,
  DEFAULT_REPO,
  DEFAULT_PLATFORM,
  parseBuildArgs,
  resolvePlan,
  buildService,
  buildImages,
  pruneBuildCache,
};
