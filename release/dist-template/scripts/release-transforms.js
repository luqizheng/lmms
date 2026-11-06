// Pure transforms extracted from release.sh so they can be unit-tested with
// node:test. Each exported function is a string-in / string-out (or
// data-in / data-out) transform with no I/O side effects; the CLI wrapper
// below reads/writes files, but the pure fns underneath stay portable.
//
// Callable as a CLI from release.sh — see `main()` at the bottom for the
// verb table (remap-volume / manifest / rewrite-agents / resolve-slug).

'use strict';

const fs = require('node:fs');
const path = require('node:path');

// ─── resolveSlug ──────────────────────────────────────────────────────────
// Resolve `services/<svc>/config` → `./config/<slug>`. Falls back to
// stripping "-service" suffix when the svc is not explicitly mapped.
// Callers pass a slug map to keep the fn pure; release.sh owns the map.
function resolveSlug(svc, slugMap) {
  if (!svc || typeof svc !== 'string') {
    throw new TypeError(`resolveSlug: svc must be a non-empty string, got ${JSON.stringify(svc)}`);
  }
  if (Object.prototype.hasOwnProperty.call(slugMap || {}, svc)) {
    return slugMap[svc];
  }
  return svc.endsWith('-service') ? svc.slice(0, -'-service'.length) : svc;
}

// ─── remapServiceVolumes ──────────────────────────────────────────────────
// Rewrite every `../services/<svc>/config` bind-mount source in a compose
// yaml to `./config/<slug>`. Container-side path (and any trailing `:ro`)
// is preserved verbatim. Returns { text, changes: [{svc, slug, matches}] }.
// Services with zero matches in the input are still reported (matches: 0)
// so callers can log "not found in compose".
function remapServiceVolumes(composeText, services, slugMap) {
  if (typeof composeText !== 'string') {
    throw new TypeError('remapServiceVolumes: composeText must be a string');
  }
  if (!Array.isArray(services)) {
    throw new TypeError('remapServiceVolumes: services must be an array of service names');
  }
  let text = composeText;
  const changes = [];
  for (const svc of services) {
    const slug = resolveSlug(svc, slugMap);
    const distPrefix = `./config/${slug}`;
    // 2026-11-06 起组 compose 进自包含子目录，dev mount 写作 ../../services/...；
    // 旧扁平形态仍是 ../services/...。先替换长者，避免 `../../services/` 被
    // `../services/` 劈成 `../ + ./config/...`（静默产出逃逸 bundle 的路径）。
    const devPrefixes = [`../../services/${svc}/config`, `../services/${svc}/config`];
    let matches = 0;
    for (const devPrefix of devPrefixes) {
      const n = countOccurrences(text, devPrefix);
      if (n > 0) {
        text = text.split(devPrefix).join(distPrefix);
        matches += n;
      }
    }
    changes.push({ svc, slug, matches });
  }
  return { text, changes };
}

function countOccurrences(haystack, needle) {
  if (!needle) return 0;
  let n = 0;
  let i = haystack.indexOf(needle);
  while (i !== -1) {
    n++;
    i = haystack.indexOf(needle, i + needle.length);
  }
  return n;
}

// ─── buildManifest ────────────────────────────────────────────────────────
// Assemble the release MANIFEST.json body. Kept as a JS object so tests can
// assert on shape without regex; the CLI serializes with 2-space indent.
function buildManifest({
  version,
  registry,
  builtAt,
  gitSha = 'unknown',
  gitBranch = 'unknown',
  exportImages,
  images,
  target,                  // optional: 'airgap' | 'deploy' | undefined. Omitted from output when falsy.
}) {
  if (!version) throw new Error('buildManifest: version is required');
  if (!registry) throw new Error('buildManifest: registry is required');
  if (!builtAt) throw new Error('buildManifest: builtAt is required');
  if (typeof exportImages !== 'boolean' && exportImages !== 0 && exportImages !== 1) {
    throw new TypeError('buildManifest: exportImages must be boolean or 0/1');
  }
  if (!Array.isArray(images)) {
    throw new TypeError('buildManifest: images must be an array');
  }
  if (target !== undefined && !['airgap', 'deploy'].includes(target)) {
    throw new TypeError(`buildManifest: target must be 'airgap' or 'deploy' when provided, got ${JSON.stringify(target)}`);
  }
  const out = {
    version,
    registry,
    built_at: builtAt,
    git_sha: gitSha,
    git_branch: gitBranch,
    export_images: exportImages === true || exportImages === 1 ? 1 : 0,
    images: [...images],
  };
  if (target) out.target = target;
  return out;
}

// ─── rewriteAgentsMd ──────────────────────────────────────────────────────
// Rewrite the top-level AGENTS.md when copied into the dist:
//   1. `.trae/rules/`  → `.agents/rules/`
//   2. `.trae/rule/`   → `.agents/rules/` (typo in dev)
//   3. `.trae/skills/` → `.agents/skills/`
//   4. Drop rows in the skills table that reference `.trae/skills/` (they
//      are dev-only platform skills, absent from the dist).
//   5. When `downstreamSkills` (whitelist) is provided, drop every table row
//      that references `.agents/skills/<name>` outside the whitelist — the
//      dist ships only the whitelisted scaffold skills, so the bundled
//      AGENTS.md must not point at skills the package does not contain.
//      This runs AFTER the prefix rewrites so rows that started as
//      `.trae/skills/` are normalized to `.agents/skills/` first and are
//      filtered by the same rule.
//
// Order matters: skip-rows must run BEFORE prefix rewrites, otherwise the
// prefix rewrite would already have turned `.trae/skills/` into
// `.agents/skills/` and the drop pattern would miss.
function rewriteAgentsMd(md, downstreamSkills) {
  if (typeof md !== 'string') {
    throw new TypeError('rewriteAgentsMd: md must be a string');
  }
  const dropPatterns = [
    /^\|\s*同步基础镜像\s*\|.*\.trae\/skills\/.*\r?\n/gm,
    /^\|\s*审计服务规范\s*\|.*\.trae\/skills\/.*\r?\n/gm,
    /^\|\s*引导开发环境\s*\|.*\.trae\/skills\/.*\r?\n/gm,
  ];
  let out = md;
  for (const re of dropPatterns) {
    out = out.replace(re, '');
  }
  out = out
    .split('.trae/rules/').join('.agents/rules/')
    .split('.trae/rule/').join('.agents/rules/')
    .split('.trae/skills/').join('.agents/skills/');
  if (Array.isArray(downstreamSkills) && downstreamSkills.length > 0) {
    const escapeRegExp = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const allowed = downstreamSkills.map(escapeRegExp).join('|');
    // Table rows only (`^|`); the name must be followed by a closing
    // backtick, `|`, whitespace, or end-of-row so `coder-new-service` never
    // accidentally shields `coder-new-service-extra`.
    const dropNonWhitelisted = new RegExp(
      '^\\|[^\\n]*\\.agents\\/skills\\/(?!' +
        allowed +
        '(?:[`|\\s]|$))[a-z0-9_-]+[^\\n]*\\r?\\n',
      'gm'
    );
    out = out.replace(dropNonWhitelisted, '');
  }
  return out;
}

// ─── filterCargoWorkspaceMembers ──────────────────────────────────────────
// Drop workspace member entries for `services/*`, `sdk/rust/*`, and `libs/*`
// so the dist Cargo.toml does not point at directories the user does not
// have. Preserves everything else byte-for-byte.
function filterCargoWorkspaceMembers(cargoToml) {
  if (typeof cargoToml !== 'string') {
    throw new TypeError('filterCargoWorkspaceMembers: cargoToml must be a string');
  }
  const skipLine = /^\s*"(services|sdk\/rust|libs)\//;
  return cargoToml
    .split('\n')
    .filter(line => !skipLine.test(line))
    .join('\n');
}

// ─────────────────────────────────────────────────────────────────────────
// CLI dispatch — release.sh calls `node release-transforms.js <verb> ...`
// ─────────────────────────────────────────────────────────────────────────
function main(argv) {
  const [verb, ...rest] = argv;
  const verbs = {
    'remap-volume': cliRemapVolume,
    'manifest': cliManifest,
    'rewrite-agents': cliRewriteAgents,
    'filter-cargo-members': cliFilterCargoMembers,
    'resolve-slug': cliResolveSlug,
  };
  if (!verb || !verbs[verb]) {
    console.error(`Usage: node release-transforms.js <verb> [args]`);
    console.error(`Verbs: ${Object.keys(verbs).join(', ')}`);
    process.exit(2);
  }
  return verbs[verb](rest);
}

function parseKv(args) {
  // Very small --key=val / --key val parser. Bash caller controls format
  // so we don't need commander here.
  const out = {};
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (!a.startsWith('--')) continue;
    if (a.includes('=')) {
      const [k, v] = a.slice(2).split(/=(.+)/);
      out[k] = v;
    } else {
      out[a.slice(2)] = args[++i];
    }
  }
  return out;
}

// remap-volume --file <path> --services iam-service,notification-service,token-service
//              [--slug-map iam-service=iam,notification-service=notification,...]
function cliRemapVolume(args) {
  const opts = parseKv(args);
  if (!opts.file) throw new Error('remap-volume: --file <compose-yaml> required');
  if (!opts.services) throw new Error('remap-volume: --services <csv> required');
  const services = opts.services.split(',').map(s => s.trim()).filter(Boolean);
  const slugMap = {};
  if (opts['slug-map']) {
    for (const pair of opts['slug-map'].split(',')) {
      const [k, v] = pair.split('=');
      if (k && v) slugMap[k.trim()] = v.trim();
    }
  }
  const before = fs.readFileSync(opts.file, 'utf8');
  const { text, changes } = remapServiceVolumes(before, services, slugMap);
  fs.writeFileSync(opts.file, text);
  // Print as machine-readable JSON so release.sh can parse it and log.
  process.stdout.write(JSON.stringify({ changes }) + '\n');
}

// manifest --out <path> --version v1 --registry ... --built-at ... --git-sha ...
//          --git-branch ... --export-images 0|1 --images-file <path>
function cliManifest(args) {
  const opts = parseKv(args);
  if (!opts.out) throw new Error('manifest: --out required');
  if (!opts['images-file']) throw new Error('manifest: --images-file required');
  const imagesRaw = fs.readFileSync(opts['images-file'], 'utf8');
  const images = imagesRaw.split('\n').map(l => l.trim()).filter(Boolean);
  const manifest = buildManifest({
    version: opts.version,
    registry: opts.registry,
    builtAt: opts['built-at'],
    gitSha: opts['git-sha'] || 'unknown',
    gitBranch: opts['git-branch'] || 'unknown',
    exportImages: opts['export-images'] === '1' || opts['export-images'] === 'true',
    images,
  });
  fs.writeFileSync(opts.out, JSON.stringify(manifest, null, 2) + '\n');
}

// rewrite-agents --file <path> [--downstream-skills <csv>]
function cliRewriteAgents(args) {
  const opts = parseKv(args);
  if (!opts.file) throw new Error('rewrite-agents: --file required');
  const downstreamSkills = (opts['downstream-skills'] || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  const before = fs.readFileSync(opts.file, 'utf8');
  fs.writeFileSync(opts.file, rewriteAgentsMd(before, downstreamSkills));
}

// filter-cargo-members --in <path> --out <path>
function cliFilterCargoMembers(args) {
  const opts = parseKv(args);
  if (!opts.in) throw new Error('filter-cargo-members: --in required');
  if (!opts.out) throw new Error('filter-cargo-members: --out required');
  const before = fs.readFileSync(opts.in, 'utf8');
  fs.writeFileSync(opts.out, filterCargoWorkspaceMembers(before));
}

// resolve-slug --svc <name> [--slug-map ...]
function cliResolveSlug(args) {
  const opts = parseKv(args);
  const slugMap = {};
  if (opts['slug-map']) {
    for (const pair of opts['slug-map'].split(',')) {
      const [k, v] = pair.split('=');
      if (k && v) slugMap[k.trim()] = v.trim();
    }
  }
  process.stdout.write(resolveSlug(opts.svc, slugMap) + '\n');
}

// Exports for node:test consumers.
module.exports = {
  resolveSlug,
  remapServiceVolumes,
  buildManifest,
  rewriteAgentsMd,
  filterCargoWorkspaceMembers,
};

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`[release-transforms] ${e.message}`);
    process.exit(1);
  }
}
