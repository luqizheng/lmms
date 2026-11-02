'use strict';

// Rust crate publishing to a private registry (kellnr). Merged from
// the retired publish-libs (libs topology) and publish-rust-sdks
// (sdk crates). Internal path deps are temporarily rewritten to version+registry
// deps; the restore-on-failure guarantee is the contract: whatever happens,
// finally puts every touched Cargo.toml back to its original text.

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { GuardError, ExternalError } = require('./errors.js');

// Layer 0 → 1 → 2 topology, verbatim from publish-libs.js.
//
// ⚠️ 顺序约束（2026-11-03 修正）：发布时 path 依赖被改写成
// `version + registry`，cargo 必须能在 registry 上**解析到该版本**，
// 因此**依赖方必须排在被依赖方之后**。PG 改造给 common-event 加了
// digua_common_db 依赖（outbox 泛型化），原顺序（common-event 在
// common-db 之前）会让 cargo publish 报
// "failed to select a version for the requirement digua_common_db = ^1.6.0"。
// 现顺序按真实 path 依赖拓扑化，preflight 会复核（见 assertOrderRespectsDeps）。
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
  // common-db 必须在 common-event / common 之前（它们依赖它）
  'common-db',
  'common',
  'common-event',
];

// Base crate first, then service SDKs — verbatim from publish-rust-sdks.js.
const SDKS_ORDER = [
  'digua-http-client',
  'audit-service-sdk',
  'config-service-sdk',
  'file-service-sdk',
  'iam-service-sdk',
  'notification-service-sdk',
  'token-service-sdk',
];

// libs/ dir → cargo package name (Cargo.toml [package] name uses underscores).
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

// Swap `pkg = { path = "../dir" }` (A) and `pkg = { path = "../dir", ... }` (B)
// for `pkg = { version = "<v>", registry = "<registry>", ... }`. crateVersions
// is keyed by crate directory name; toPkgName maps dir → package name.
function rewritePathDeps(tomlText, { crateVersions, registry = 'kellnr', toPkgName = dirToPackage } = {}) {
  let content = tomlText;
  for (const [dir, version] of Object.entries(crateVersions ?? {})) {
    const pkgName = toPkgName(dir);

    const patternA = `${pkgName} = { path = "../${dir}" }`;
    const replacementA = `${pkgName} = { version = "${version}", registry = "${registry}" }`;
    if (content.includes(patternA)) {
      content = content.split(patternA).join(replacementA);
      continue;
    }

    // Pattern B: { path = "../dir", <rest kept as-is> }
    const reB = new RegExp(
      `${escapeRegex(pkgName)}\\s*=\\s*\\{\\s*path\\s*=\\s*"\\.\\./${escapeRegex(dir)}"\\s*,\\s*`,
    );
    const replacementB = `${pkgName} = { version = "${version}", registry = "${registry}", `;
    if (reB.test(content)) {
      content = content.replace(reB, replacementB);
    }
  }
  return content;
}

function assertPublishable(tomlText, dir) {
  for (const field of ['description', 'license']) {
    if (!new RegExp(`^${field}\\s*=`, 'm').test(tomlText)) {
      throw new GuardError('CRATE_FIELD_MISSING', `${dir} missing '${field}' in Cargo.toml — required by the registry`, { dir });
    }
  }
  const v = /^version\s*=\s*"([^"]+)"/m.exec(tomlText);
  return v ? v[1] : null;
}

/**
 * 列出某 crate manifest 里指向仓内其他 crate 的 path 依赖（目录名）。
 *
 * @param {string} tomlText crate 的 Cargo.toml 文本。
 * @param {string[]} allDirs 仓内全部 crate 目录名。
 * @param {Function} toPkgName 目录名 → 包名。
 * @param {string} [selfDir] 当前 crate 目录（自依赖不计——真实仓不会出现，
 *   但测试夹具会给每个 crate 统一加一条 common-error 依赖）。
 * @returns {string[]} 依赖的目录名列表。
 */
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

/**
 * 发布序必须尊重依赖方向：依赖方排在被依赖方**之后**（否则 cargo 无法在
 * registry 上解析到被依赖方的版本）。
 *
 * @param {Array<{dir: string, family: string, base: string}>} dirs 发布序列。
 * @param {Object} fsMod 文件系统（便于测试注入）。
 * @throws {GuardError} 顺序违规时抛出，并指名道姓列出违反对。
 */
function assertOrderRespectsDeps(dirs, fsMod = fs) {
  const allLibDirs = LIBS_ORDER;
  const seen = new Set();
  for (const { dir, family, base } of dirs) {
    if (family !== 'libs') continue;
    const tomlPath = path.join(base, dir, 'Cargo.toml');
    let text;
    try {
      text = fsMod.readFileSync(tomlPath, 'utf-8');
    } catch {
      continue; // 缺失由 preflight 的 CRATE_TOML_MISSING 报
    }
    for (const dep of internalPathDeps(text, allLibDirs, dirToPackage, dir)) {
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

function cargoPublishArgs(pkgName, registry, dryRun) {
  const args = ['publish', '-p', pkgName, '--registry', registry, '--no-verify', '--allow-dirty'];
  if (dryRun) args.push('--dry-run');
  return args;
}

function publishRust({
  root,
  version,
  registry = 'kellnr',
  targets = 'all',
  dryRun = false,
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 }),
  out = { log: (m) => console.log(m), warn: (m) => console.warn(m) },
  fs: nodeFs = fs,
} = {}) {
  if (!version && !dryRun) throw new GuardError('RUST_VERSION_REQUIRED', 'a version argument is required (e.g. 1.7.0)');
  const sequences = targets === 'libs' ? { libs: LIBS_ORDER }
    : targets === 'sdks' ? { sdks: SDKS_ORDER }
    : { libs: LIBS_ORDER, sdks: SDKS_ORDER };

  const dirs = [];
  for (const [family, order] of Object.entries(sequences)) {
    for (const dir of order) {
      dirs.push({ dir, family, base: family === 'libs' ? 'libs' : path.join('sdk', 'rust') });
    }
  }

  const plan = dirs.map(({ dir, family }) => `${dir} (${family})`);
  if (dryRun) {
    out.log(`[rust-publish] dry-run plan (registry=${registry}, version=${version}):`);
    for (const line of plan) out.log(`  - ${line}`);
    return { published: [], plan, dryRun: true };
  }

  // Cargo.lock 快照必须在任何 exec 之前取：`cargo --version` 探活等前置动作
  // 理论上也可能触碰 lock 文件（实测场景：探活之后的任何写入都会污染快照）。
  // Cargo.lock 也必须复原：发布时 path 依赖被临时改成 registry 依赖，cargo 会把
  // 「kellnr 双版本条目」写进 lock（实测：digua_common_error 1.6.0 同时出现 sparse
  // 与 192.168.2.12:16444 两条来源）。只复原 Cargo.toml 会留下这个污染。
  const lockPath = path.join(root, 'Cargo.lock');
  let lockOriginal = null;
  try {
    lockOriginal = nodeFs.readFileSync(lockPath, 'utf-8');
  } catch {
    lockOriginal = null; // 无 lock（单 crate 仓）则无需复原
  }

  const res = exec('cargo', ['--version'], { stdio: 'ignore' });
  if (res.error) throw new GuardError('RUST_CARGO_MISSING', "'cargo' not found on PATH");

  // Pre-flight: every crate exists, is publishable, and already carries the target version.
  for (const { dir, base } of dirs) {
    const tomlPath = path.join(root, base, dir, 'Cargo.toml');
    let text;
    try {
      text = nodeFs.readFileSync(tomlPath, 'utf-8');
    } catch {
      throw new GuardError('CRATE_TOML_MISSING', `${base}/${dir}/Cargo.toml not found`, { dir });
    }
    assertPublishable(text, dir);
    const current = assertPublishable(text, dir) && /^version\s*=\s*"([^"]+)"/m.exec(text)[1];
    if (current !== version) {
      throw new GuardError('CRATE_VERSION_MISMATCH', `${dir} Cargo.toml version ${current} != target ${version} — bump crates first`, { dir, current, target: version });
    }
  }

  // 发布序必须尊重依赖方向（依赖方在依赖之后发布）。
  assertOrderRespectsDeps(
    dirs.map(({ dir, family, base }) => ({ dir, family, base: path.join(root, base) })),
    nodeFs,
  );

  // crateVersions covers the whole published set (all families) so cross-family
  // path deps rewrite too.
  const crateVersions = Object.fromEntries(dirs.map(({ dir }) => [dir, version]));
  const originals = new Map();
  const published = [];
  const skipped = [];
  const toPkg = (dir, family) => (family === 'libs' ? dirToPackage(dir) : dir);

  try {
    for (const { dir, family, base } of dirs) {
      const tomlPath = path.join(root, base, dir, 'Cargo.toml');
      const before = nodeFs.readFileSync(tomlPath, 'utf-8');
      const after = rewritePathDeps(before, { crateVersions, registry });
      if (after !== before && !originals.has(tomlPath)) originals.set(tomlPath, before);
      if (after !== before) nodeFs.writeFileSync(tomlPath, after, 'utf-8');

      const pkgName = toPkg(dir, family);
      out.log(`[rust-publish] publishing ${dir} (pkg: ${pkgName})`);
      const result = exec('cargo', cargoPublishArgs(pkgName, registry, false), { cwd: path.join(root, base) });
      if ((result.status ?? 1) !== 0) {
        const stderr = String(result.stderr || '');
        // 断点续发：该版本已在此 registry 上（半途失败的发布序列重跑时必现，
        // 以及「部分 crate 已发、其余未发」的版本对齐场景）。
        // 「已存在」不是失败——跳过它，继续后续 crate。
        if (/already exists|already uploaded|is already present/i.test(stderr)) {
          out.warn(`[rust-publish] ${pkgName} ${version} 已在 registry（跳过）`);
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
    for (const [tomlPath, text] of originals) {
      nodeFs.writeFileSync(tomlPath, text, 'utf-8');
    }
    if (lockOriginal !== null) {
      try {
        nodeFs.writeFileSync(lockPath, lockOriginal, 'utf-8');
      } catch {
        // lock 复原失败不应掩盖真正的发布错误
      }
    }
  }
}

module.exports = { LIBS_ORDER, SDKS_ORDER, dirToPackage, rewritePathDeps, publishRust, internalPathDeps, assertOrderRespectsDeps };
