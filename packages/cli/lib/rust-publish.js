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
const LIBS_ORDER = [
  'common-error',
  'common-health',
  'common-jwt',
  'common-ratelimit',
  'common-telemetry',
  'common-cache',
  'common-config',
  'common-crypto',
  'common-event',
  'common-http',
  'common-log',
  'common-validator',
  'common-telemetry-axum',
  'jwt-ctx',
  'common-db',
  'authz',
  'common',
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

  // crateVersions covers the whole published set (all families) so cross-family
  // path deps rewrite too.
  const crateVersions = Object.fromEntries(dirs.map(({ dir }) => [dir, version]));
  const originals = new Map();
  const published = [];
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
        throw new ExternalError(`cargo publish failed for ${dir}`, {
          dir,
          stderr: String(result.stderr || '').split('\n').slice(0, 5).join('\n'),
        });
      }
      published.push({ name: pkgName, version });
    }
    return { published, plan };
  } finally {
    for (const [tomlPath, text] of originals) {
      nodeFs.writeFileSync(tomlPath, text, 'utf-8');
    }
  }
}

module.exports = { LIBS_ORDER, SDKS_ORDER, dirToPackage, rewritePathDeps, publishRust };
