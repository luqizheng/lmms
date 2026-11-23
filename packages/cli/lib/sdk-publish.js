'use strict';

// sdk-publish.js —— 发布各类 SDK（Rust crates / npm TS 包 / Java SDK）
//
// 三个生态各自独立版本线（用户裁决：不跟平台镜像版本绑定）：
//   rust  : libs/*（17）→ sdk/rust/*（6）发 crates.io，按依赖拓扑排序
//   npm   : packages/@digua/* + sdk/*-services 发 npmjs.org
//   java  : sdk/java 多模块发 Maven（mvn deploy）
//
// Rust 的关键约束（实战验证过，勿简化）：
//   1. 发布时把 `path = "../x"` 依赖临时改写成 `version + registry`，
//      否则 cargo 无法在被依赖方尚未上 registry 时解析依赖；
//   2. 依赖方必须排在被依赖方之后（否则 registry 上还没有该版本）；
//   3. **无论成败**都要把 Cargo.toml + Cargo.lock 复原（finally）；
//   4. 目标版本已存在 → 视为跳过而非失败（支持断点续发）。

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { GuardError, ExternalError } = require('./errors.js');
const { readWorkspaceVersion } = require('./service-versions.js');

// ── Rust 发布拓扑 ────────────────────────────────────────────────────────────
// ⚠️ 依赖方必须排在被依赖方之后。common-db 必须在 common-event / common 之前
// （它们依赖它）—— 顺序错了 cargo publish 会报 "failed to select a version"。
const LIBS_ORDER = [
  'common-error',
  'common-health',
  'common-jwt',
  'common-ratelimit',
  'common-telemetry',
  'common-cache',
  'common-config',
  'common-crypto',
  'common-http',
  'common-log',
  'common-telemetry-axum',
  'common-validator',
  'jwt-ctx',
  'authz',
  'common-db',
  'common',
  'common-event',
];

const SDKS_ORDER = [
  'digua-http-client',
  'audit-service-sdk',
  'file-service-sdk',
  'iam-service-sdk',
  'notification-service-sdk',
  'token-service-sdk',
];

function dirToPackage(dir) {
  switch (dir) {
    case 'authz': return 'digua_authz';
    case 'jwt-ctx': return 'digua_jwt_ctx';
    case 'common': return 'digua_common';
    default: return 'digua_' + dir.replaceAll('-', '_');
  }
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// `pkg = { path = "../dir" }`（含额外字段）→ `pkg = { version = "<v>", registry = "<registry>", ... }`
function rewritePathDeps(tomlText, { crateVersions, registry = 'kellnr', toPkgName = dirToPackage, pkgNames = null } = {}) {
  let content = tomlText;
  const nameOf = (dir) => pkgNames?.[dir] ?? toPkgName(dir);

  for (const [dir, version] of Object.entries(crateVersions ?? {})) {
    const pkgName = nameOf(dir);

    const patternA = `${pkgName} = { path = "../${dir}" }`;
    const replacementA = registry
      ? `${pkgName} = { version = "${version}", registry = "${registry}" }`
      : `${pkgName} = { version = "${version}" }`;
    if (content.includes(patternA)) {
      content = content.split(patternA).join(replacementA);
      continue;
    }

    const reB = new RegExp(
      `${escapeRegex(pkgName)}\\s*=\\s*\\{\\s*path\\s*=\\s*"\\.\\./${escapeRegex(dir)}"\\s*,\\s*`,
    );
    const replacementB = registry
      ? `${pkgName} = { version = "${version}", registry = "${registry}", `
      : `${pkgName} = { version = "${version}", `;
    if (reB.test(content)) content = content.replace(reB, replacementB);
  }
  return content;
}

// 指到仓内其他 crate 的 path 依赖（目录名）
function internalPathDeps(tomlText, allDirs, toPkgName = dirToPackage, selfDir = null) {
  const deps = [];
  for (const dir of allDirs) {
    if (dir === selfDir) continue;
    const name = toPkgName(dir);
    const re = new RegExp(`^${escapeRegex(name)}\\s*=\\s*\\{[^}]*path\\s*=\\s*"\\.\\./${escapeRegex(dir)}"`, 'm');
    if (re.test(tomlText)) deps.push(dir);
  }
  return deps;
}

// 发布序必须尊重依赖方向：依赖方排在被依赖方之后
function assertOrderRespectsDeps(dirs, fsMod = fs) {
  const seen = new Set();
  for (const { dir, family, base } of dirs) {
    if (family !== 'libs') continue;
    let text;
    try {
      text = fsMod.readFileSync(path.join(base, dir, 'Cargo.toml'), 'utf-8');
    } catch {
      continue; // 缺失由 preflight 报
    }
    for (const dep of internalPathDeps(text, LIBS_ORDER, dirToPackage, dir)) {
      if (!seen.has(dep)) {
        throw new GuardError(
          'CRATE_ORDER_VIOLATION',
          `publish order: ${dir} depends on ${dep}, but ${dep} is published later — ` +
            'the dependency version would not exist on the registry yet',
          { dir, dep, order: dirs.map((d) => d.dir) },
        );
      }
    }
    seen.add(dir);
  }
}

// 从 manifest 读 package name + version（支持 version.workspace = true）
function readCrateManifest(root, base, dir) {
  const file = path.join(root, base, dir, 'Cargo.toml');
  if (!fs.existsSync(file)) throw new GuardError('CRATE_TOML_MISSING', `${base}/${dir}/Cargo.toml not found`, { dir });
  const text = fs.readFileSync(file, 'utf-8');

  const name = /^name\s*=\s*"([^"]+)"/m.exec(text)?.[1] ?? null;
  let version = /^version\s*=\s*"([^"]+)"/m.exec(text)?.[1] ?? null;
  if (!version && /^version\.workspace\s*=\s*true/m.test(text)) version = readWorkspaceVersion(root);
  if (!version) throw new GuardError('CRATE_VERSION_MISSING', `${base}/${dir} has no resolvable version`, { dir });

  for (const field of ['description', 'license']) {
    if (!new RegExp(`^${field}\\s*=`, 'm').test(text)) {
      throw new GuardError('CRATE_FIELD_MISSING', `${dir} missing '${field}' in Cargo.toml — required by the registry`, { dir });
    }
  }
  return { name, version, text };
}

function cargoPublishArgs(pkgName, registry, dryRun) {
  const args = ['publish', '-p', pkgName, '--no-verify', '--allow-dirty'];
  if (registry) args.push('--registry', registry);
  if (dryRun) args.push('--dry-run');
  return args;
}

// 发布 Rust crates。targets: all | libs | sdks
function publishRust({
  root,
  registry = 'kellnr',
  targets = 'all',
  dryRun = false,
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 }),
  out = { log: console.log, warn: console.warn },
  fs: nodeFs = fs,
} = {}) {
  const sequences = targets === 'libs' ? { libs: LIBS_ORDER }
    : targets === 'sdks' ? { sdks: SDKS_ORDER }
    : { libs: LIBS_ORDER, sdks: SDKS_ORDER };

  const dirs = [];
  for (const [family, order] of Object.entries(sequences)) {
    for (const dir of order) {
      dirs.push({ dir, family, base: family === 'libs' ? 'libs' : path.join('sdk', 'rust') });
    }
  }

  // Preflight：读真实包名与版本（目录名 ≠ 包名；版本各 crate 独立）
  const pkgNames = {};
  const crateVersions = {};
  for (const { dir, base } of dirs) {
    const m = readCrateManifest(root, base, dir);
    pkgNames[dir] = m.name;
    crateVersions[dir] = m.version;
  }
  assertOrderRespectsDeps(dirs.map(({ dir, family, base }) => ({ dir, family, base: path.join(root, base) })), nodeFs);

  const plan = dirs.map(({ dir, family }) => `${dir} (${family}) → ${pkgNames[dir]}@${crateVersions[dir]}`);
  if (dryRun) {
    out.log(`[sdk:rust] dry-run plan (registry=${registry}):`);
    for (const line of plan) out.log(`  - ${line}`);
    return { published: [], skipped: [], plan, dryRun: true };
  }

  // Cargo.lock 快照必须在任何 exec 之前取（探活之后的写入会污染快照）。
  // 发布时 path 依赖被临时改成 registry 依赖，cargo 会把「双来源条目」写进 lock。
  const lockPath = path.join(root, 'Cargo.lock');
  let lockOriginal = null;
  try { lockOriginal = nodeFs.readFileSync(lockPath, 'utf-8'); } catch { lockOriginal = null; }

  const res = exec('cargo', ['--version'], { stdio: 'ignore' });
  if (res.error) throw new GuardError('RUST_CARGO_MISSING', "'cargo' not found on PATH");

  const originals = new Map();
  const published = [];
  const skipped = [];

  try {
    for (const { dir, family, base } of dirs) {
      const tomlPath = path.join(root, base, dir, 'Cargo.toml');
      const before = nodeFs.readFileSync(tomlPath, 'utf-8');
      const after = rewritePathDeps(before, { crateVersions, registry, pkgNames });
      if (after !== before && !originals.has(tomlPath)) originals.set(tomlPath, before);
      if (after !== before) nodeFs.writeFileSync(tomlPath, after, 'utf-8');

      const pkgName = pkgNames[dir];
      const version = crateVersions[dir];
      out.log(`[sdk:rust] publishing ${dir} (${pkgName}@${version})`);
      const result = exec('cargo', cargoPublishArgs(pkgName, registry, false), { cwd: path.join(root, base) });
      if ((result.status ?? 1) !== 0) {
        const stderr = String(result.stderr || '');
        // 断点续发：该版本已在此 registry 上（半途失败的发布序列重跑时必现）
        if (/already exists|already uploaded|is already present/i.test(stderr)) {
          out.warn(`[sdk:rust] ${pkgName} ${version} 已在 registry（跳过）`);
          skipped.push({ name: pkgName, version, reason: 'already exists' });
          continue;
        }
        throw new ExternalError(`cargo publish failed for ${dir}`, {
          dir,
          stderr: stderr.split('\n').slice(0, 5).join('\n'),
        });
      }
      published.push({ name: pkgName, version });
    }
    return { published, skipped, plan };
  } finally {
    for (const [tomlPath, text] of originals) nodeFs.writeFileSync(tomlPath, text, 'utf-8');
    if (lockOriginal !== null) {
      try { nodeFs.writeFileSync(lockPath, lockOriginal, 'utf-8'); } catch { /* 不掩盖真正的发布错误 */ }
    }
  }
}

// ── npm 发布 ────────────────────────────────────────────────────────────────
// 顺序：contracts → http → 各 svc SDK（SDK 的构建产物内联依赖，包间无 deps 约束）
const NPM_PACKAGES = [
  { dir: 'packages/contracts', name: '@digua/contracts', needsBuild: true },
  { dir: 'packages/http', name: '@digua/http', needsBuild: true },
  // 全部使用 @digua/ scope：未加 scope 的 file-services / notification-services
  // 在 npmjs.org 已被他人占用，无法发布；统一 scope 也便于 org 权限管理。
  { dir: 'sdk/iam-services', name: '@digua/iam-services', needsBuild: true },
  { dir: 'sdk/audit-services', name: '@digua/audit-services', needsBuild: true },
  { dir: 'sdk/file-services', name: '@digua/file-services', needsBuild: true },
  { dir: 'sdk/notification-services', name: '@digua/notification-services', needsBuild: true },
  { dir: 'sdk/prue-services', name: '@digua/prue-services', needsBuild: true },
];

// Windows 上 pnpm 是 .cmd shim，spawnSync 不带 shell 解析不到
const IS_WIN = process.platform === 'win32';

function publishNpm({
  root,
  registry = null,
  dryRun = false,
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, encoding: 'utf-8' }),
  out = { log: console.log, warn: console.warn },
} = {}) {
  const plan = NPM_PACKAGES.map((p) => {
    const pkg = JSON.parse(fs.readFileSync(path.join(root, p.dir, 'package.json'), 'utf8'));
    return { ...p, version: pkg.version, private: pkg.private === true };
  });

  if (dryRun) {
    out.log('[sdk:npm] dry-run plan:');
    for (const p of plan) out.log(`  - ${p.name}@${p.version}${p.private ? '（private，跳过）' : ''}`);
    return { published: [], skipped: [], plan, dryRun: true };
  }

  const published = [];
  const skipped = [];
  for (const p of plan) {
    if (p.private) { skipped.push({ name: p.name, reason: 'private' }); continue; }

    if (p.needsBuild) {
      out.log(`[sdk:npm] build ${p.name}`);
      const b = exec('pnpm', ['--filter', p.name, 'run', 'build'], { cwd: root, shell: IS_WIN });
      if ((b.status ?? 1) !== 0) throw new ExternalError(`pnpm build failed for ${p.name}`, { name: p.name });
    }

    const args = ['--filter', p.name, 'publish', '--no-git-checks', '--access', 'public'];
    if (registry) args.push('--registry', registry);
    out.log(`[sdk:npm] publishing ${p.name}@${p.version}`);
    const r = exec('pnpm', args, { cwd: root, shell: IS_WIN });
    if ((r.status ?? 1) !== 0) {
      const stderr = String(r.stderr || '');
      if (/previously published|EPUBLISHCONFLICT|cannot publish over/i.test(stderr)) {
        out.warn(`[sdk:npm] ${p.name}@${p.version} 已存在（跳过）`);
        skipped.push({ name: p.name, version: p.version, reason: 'already published' });
        continue;
      }
      throw new ExternalError(`pnpm publish failed for ${p.name}`, {
        name: p.name,
        stderr: stderr.split('\n').slice(0, 5).join('\n'),
      });
    }
    published.push({ name: p.name, version: p.version });
  }
  return { published, skipped, plan };
}

// ── Java 发布 ───────────────────────────────────────────────────────────────
function publishMaven({
  root,
  repositoryId = null,
  dryRun = false,
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, encoding: 'utf-8' }),
  out = { log: console.log, warn: console.warn },
} = {}) {
  const pom = path.join(root, 'sdk', 'java', 'pom.xml');
  if (!fs.existsSync(pom)) throw new GuardError('MAVEN_POM_MISSING', `sdk/java/pom.xml not found（Java SDK 可能未初始化）`);
  const version = /<version>([^<]+)<\/version>/.exec(fs.readFileSync(pom, 'utf8'))?.[1] ?? 'unknown';

  const plan = [`sdk/java → ${version}`];
  if (dryRun) {
    out.log(`[sdk:java] dry-run plan: ${plan[0]}${repositoryId ? ` → ${repositoryId}` : ''}`);
    return { published: [], skipped: [], plan, dryRun: true };
  }

  const args = ['-f', pom, 'deploy', '-DskipTests'];
  if (repositoryId) args.push(`-DaltDeploymentRepository=${repositoryId}::default::${repositoryId}`);
  out.log(`[sdk:java] mvn deploy ${version}`);
  const r = exec('mvn', args, { cwd: root, shell: IS_WIN });
  if ((r.status ?? 1) !== 0) {
    throw new ExternalError('mvn deploy failed', { stderr: String(r.stderr || '').split('\n').slice(0, 5).join('\n') });
  }
  return { published: [{ name: 'digua-sdk', version }], skipped: [], plan };
}

module.exports = {
  LIBS_ORDER,
  SDKS_ORDER,
  NPM_PACKAGES,
  dirToPackage,
  rewritePathDeps,
  internalPathDeps,
  assertOrderRespectsDeps,
  readCrateManifest,
  cargoPublishArgs,
  publishRust,
  publishNpm,
  publishMaven,
};
