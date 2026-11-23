'use strict';

// coder release —— 平台发布（两条线：服务镜像 / SDK 包）
//
//   coder release status [--strict]         版本体检（服务版本 / .image.env / registry / git tag）
//   coder release images [--push] [--dry-run]
//                                           按 services/* 的 crate 版本构建镜像；
//                                           --push 成功后自动更新 .image.env
//   coder release sdk [rust|npm|java|all] [--dry-run] [--targets libs|sdks]
//                                           发布各生态 SDK（版本各自独立）
//
// 版本真相源：根 Cargo.toml 的 [workspace.package].version（8 个 services 继承）
// 镜像参数：  release.toml（dialect / registry）
// 部署钉版：  examples/docker-deploy/.image.env（由 release images 自动写）
//
// 设计见 docs/superpowers/specs/2026-11-09-image-env-driven-deploy-design.md。

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { UsageError, GuardError } = require('../lib/errors.js');
const { parseFlags } = require('../lib/args.js');
const { readReleaseConfig } = require('../lib/release-toml.js');
const { readServiceVersions } = require('../lib/service-versions.js');
const imageEnv = require('../lib/image-env.js');
const { resolvePlan, buildImages } = require('../lib/docker-build.js');
const sdkPublish = require('../lib/sdk-publish.js');

const DEPLOY_DIR = path.join('examples', 'docker-deploy');

// ── git tag 侧 ───────────────────────────────────────────────────────────────
function parseSemver(v) {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(String(v || '').trim());
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

function latestGitTag(root) {
  try {
    const out = execFileSync('git', ['tag', '-l', 'v*'], { cwd: root, encoding: 'utf-8' });
    let best = null;
    for (const t of out.split(/\r?\n/).map((x) => x.trim()).filter(Boolean)) {
      const p = parseSemver(t);
      if (!p) continue;
      if (!best || p[0] > best.p[0] || (p[0] === best.p[0] && (p[1] > best.p[1] || (p[1] === best.p[1] && p[2] > best.p[2])))) {
        best = { tag: t, p };
      }
    }
    return best ? best.tag : null;
  } catch {
    return null;
  }
}

// ── 部署单元钉版（.image.env）当前版本 ───────────────────────────────────────
// 每个部署单元各一份 .image.env；返回 [{ unit, dir, version }]
function pinnedVersions(root) {
  return imageEnv.DEPLOY_UNITS.map((unit) => {
    const file = path.join(root, unit.dir, imageEnv.ENV_FILE);
    const derived = fs.existsSync(file) ? imageEnv.deriveVersionDialect(file) : null;
    return { unit: unit.dir, file, version: derived ? derived.version : null };
  });
}

// 服务版本必须一致，否则拒绝发布（避免同批镜像两个 tag）
function requireConsistentVersion(root) {
  const versions = readServiceVersions(root);
  if (versions.missing.length) {
    throw new GuardError(
      'SERVICE_MANIFEST_MISSING',
      `服务 Cargo.toml 缺失：${versions.missing.map((m) => m.dir).join(', ')}`,
      { missing: versions.missing },
    );
  }
  if (versions.splits.length) {
    const detail = versions.splits.map((s) => `${s.dir}=${s.version}`).join(', ');
    throw new GuardError(
      'VERSION_SPLIT',
      `服务版本不一致（期望 ${versions.version}）：${detail} —— 统一改用 version.workspace = true，或改根 Cargo.toml 的 [workspace.package].version`,
      { expected: versions.version, splits: versions.splits },
    );
  }
  return versions;
}

const commands = {
  'release status': {
    summary: 'version health check: service crates / .image.env pins / git tag',
    spec: {
      summary: 'version health check: service crates / .image.env pins / git tag',
      usage: 'coder release status [--strict]',
      examples: ['coder release status', 'coder release status --strict'],
      flags: {
        '--strict': { type: 'flag', desc: 'exit 3 when versions drift apart' },
      },
    },
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release status requires a repo context (--cwd)');
      const { flags } = parseFlags(args, { '--strict': 'flag' });

      const cfg = readReleaseConfig(ctx.root);
      const versions = readServiceVersions(ctx.root);
      const tag = latestGitTag(ctx.root);
      const pinned = pinnedVersions(ctx.root);

      const drifts = [];
      if (versions.missing.length) drifts.push(`服务 Cargo.toml 缺失：${versions.missing.map((m) => m.dir).join(', ')}`);
      if (versions.splits.length) drifts.push(`服务版本分裂：${versions.splits.map((s) => `${s.dir}=${s.version}`).join(', ')}`);
      for (const p of pinned) {
        if (!p.version) drifts.push(`${p.unit}/.image.env 缺失或不可解析`);
        else if (versions.version && p.version !== versions.version) {
          drifts.push(`${p.unit}/.image.env 钉版 ${p.version} ≠ 服务版本 ${versions.version}`);
        }
      }
      if (versions.version && tag && tag.replace(/^v/, '') !== versions.version) {
        drifts.push(`最新 git tag ${tag} ≠ 服务版本 ${versions.version}`);
      }

      const data = {
        version: versions.version,
        workspaceVersion: versions.workspaceVersion,
        services: versions.services.map((s) => ({ dir: s.dir, crate: s.crate, version: s.version, inherited: s.inherited })),
        dialect: cfg.dialect,
        registry: `${cfg.registry}/${cfg.repo}`,
        gitTag: tag,
        pinned: pinned.map((p) => ({ unit: p.unit, version: p.version })),
        drifts,
        ok: drifts.length === 0,
      };
      const human = [
        `服务版本（workspace）: ${versions.workspaceVersion || '(缺失)'}`,
        `服务版本（解析后）  : ${versions.version || '(不一致)'}`,
        `方言 / 镜像源        : ${cfg.dialect} / ${cfg.registry}/${cfg.repo}`,
        `最新 git tag        : ${tag || '(无)'}`,
        ...pinned.map((p) => `钉版 ${p.unit.padEnd(28)}: ${p.version || '(无)'}`),
        ...versions.services.map((s) => `  · ${s.dir.padEnd(24)} ${s.version}${s.inherited ? '（继承）' : '（写死!）'}`),
        drifts.length ? `⚠ 问题：${drifts.join('；')}` : '✓ 版本一致',
      ];
      if (flags['--strict'] && drifts.length) {
        throw new GuardError('VERSION_DRIFT', drifts.join('；'), { drifts });
      }
      return { data, human };
    },
  },

  'release images': {
    summary: 'build (and push) service images from services/* crate versions; pin .image.env after push',
    spec: {
      summary: 'build (and push) service images from services/* crate versions; pin .image.env after push',
      usage: 'coder release images [--push] [--dry-run] [--registry <host>] [--repo <name>]',
      examples: ['coder release images --dry-run', 'coder release images --push'],
      flags: {
        '--push': { type: 'flag', desc: 'push images to the registry' },
        '--dry-run': { type: 'flag', desc: 'print the plan and resulting pins; build nothing' },
        '--registry': { type: 'value', desc: 'override registry host/prefix' },
        '--repo': { type: 'value', desc: 'override registry namespace/repository prefix' },
      },
    },
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release images requires a repo context (--cwd)');
      const { flags } = parseFlags(args, { '--push': 'flag', '--dry-run': 'flag' });

      const cfg = readReleaseConfig(ctx.root);
      const versions = requireConsistentVersion(ctx.root);
      const version = versions.version;

      // 方言 → 构建批次。
      // pg 方言下 docker-build 只构建带 DB 的服务，api-gateway（无 DB）会被跳过；
      // 但 .image.env 里 api-gateway 用的是**无后缀** tag —— 部署时照样要拉它。
      // 所以 pg 发布必须两段：先发方言无关服务（无后缀），再发 pg 变体。
      const baseArgs = {
        services: [],
        version,
        push: Boolean(flags['--push']),
        loadAndPush: false,
        platforms: null,
        registry: cfg.registry,
        repo: cfg.repo,
        buildArgs: [],
        unknown: [],
      };
      const batches = cfg.dialect === 'pg'
        ? [
            { label: 'dialect-agnostic（无后缀）', plan: resolvePlan({ args: { ...baseArgs, dialect: 'mysql', services: ['api-gateway'] }, isTTY: false }) },
            { label: 'pg（-pg 后缀）', plan: resolvePlan({ args: { ...baseArgs, dialect: 'pg' }, isTTY: false }) },
          ]
        : [{ label: 'mysql（无后缀）', plan: resolvePlan({ args: { ...baseArgs, dialect: 'mysql' }, isTTY: false }) }];

      for (const b of batches) for (const w of b.plan.warnings) ctx.out.warn(w);

      const refs = imageEnv.imageRefs({
        version,
        dialect: cfg.dialect,
        registry: cfg.registry,
        repo: cfg.repo,
      });
      const pinFiles = imageEnv.DEPLOY_UNITS.map((u) => path.join(ctx.root, u.dir, imageEnv.ENV_FILE));

      if (flags['--dry-run']) {
        const human = [
          `version : ${version}（来自 services/* crate 版本，dialect=${cfg.dialect}）`,
          `registry: ${cfg.registry}/${cfg.repo}`,
          `mode    : ${flags['--push'] ? 'push' : 'local'}`,
          ...batches.map((b) => `batch   : ${b.label} → ${b.plan.services.join(', ')}`),
          '将写入的钉版：',
          ...refs.map((r) => `  ${r.env}=${r.ref}`),
          ...pinFiles.map((f) => `  → ${f}`),
        ];
        return { data: { batches: batches.map((b) => ({ label: b.label, plan: b.plan })), refs, pinFiles, dryRun: true }, human };
      }

      const built = [];
      const failed = [];
      for (const b of batches) {
        const r = buildImages({ root: ctx.root, plan: b.plan, json: ctx.json, out: ctx.out });
        built.push(...r.built);
        failed.push(...r.failed);
      }
      const human = [
        `[done] built ${built.length}, failed ${failed.length}`,
        ...failed.map((f) => `  ✗ ${f.service}: ${f.reason}`),
      ];

      if (failed.length) {
        return { data: { built, failed, pinned: false }, human, exitCode: 1 };
      }

      // 只有真推上去了才更新钉版 —— 否则远程拉不到镜像。
      // 两个部署单元（services-only / 全栈）各写一份，基础镜像段沿用现值。
      let pinned = false;
      if (flags['--push']) {
        const written = [];
        for (const unit of imageEnv.DEPLOY_UNITS) {
          const file = path.join(ctx.root, unit.dir, imageEnv.ENV_FILE);
          const existingRaw = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
          imageEnv.writeImageEnv(file, imageEnv.buildImageEnv({
            version,
            dialect: cfg.dialect,
            registry: cfg.registry,
            repo: cfg.repo,
            includeBase: unit.base,
            existingRaw,
          }));
          written.push(file);
        }
        pinned = true;
        for (const f of written) human.push(`✓ 已更新镜像钉版：${f}`);
        human.push(`（version=${version}）下一步：提交 .image.env 的 diff，并打 git tag v${version}`);
      } else {
        human.push('（未 --push：不更新 .image.env —— 镜像还没进 registry）');
      }

      return { data: { built, failed, pinned, refs }, human };
    },
  },

  'release sdk': {
    summary: 'publish SDKs (rust crates / npm packages / java) — versions are per-ecosystem',
    spec: {
      summary: 'publish SDKs (rust crates / npm packages / java) — versions are per-ecosystem',
      usage: 'coder release sdk [rust|npm|java|all] [--dry-run] [--targets libs|sdks|all]',
      examples: [
        'coder release sdk all --dry-run',
        'coder release sdk rust --targets sdks',
        'coder release sdk npm',
      ],
      flags: {
        '--dry-run': { type: 'flag', desc: 'print the plan; publish nothing' },
        '--targets': { type: 'value', desc: 'rust 子集：libs | sdks | all（默认 all）' },
      },
      positional: { name: 'ecosystem', desc: 'rust | npm | java | all（默认 all）' },
    },
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('release sdk requires a repo context (--cwd)');
      const { flags, positional } = parseFlags(args, { '--dry-run': 'flag', '--targets': 'value' });
      const which = (positional[0] || 'all').toLowerCase();
      if (!['rust', 'npm', 'java', 'all'].includes(which)) {
        throw new UsageError(`unknown ecosystem: ${which}（允许 rust | npm | java | all）`);
      }
      const targets = (flags['--targets'] || 'all').toLowerCase();
      if (!['libs', 'sdks', 'all'].includes(targets)) {
        throw new UsageError(`--targets 只能是 libs | sdks | all（收到 ${targets}）`);
      }

      const cfg = readReleaseConfig(ctx.root);
      const dryRun = Boolean(flags['--dry-run']);
      const human = [];
      const data = { ecosystem: which, dryRun, rust: null, npm: null, java: null };

      if (which === 'rust' || which === 'all') {
        data.rust = sdkPublish.publishRust({
          root: ctx.root,
          registry: cfg.sdk.rustRegistry,
          targets,
          dryRun,
          out: ctx.out,
        });
        human.push(`rust: published ${data.rust.published.length}, skipped ${data.rust.skipped.length}`);
      }
      if (which === 'npm' || which === 'all') {
        data.npm = sdkPublish.publishNpm({
          root: ctx.root,
          registry: cfg.sdk.npmRegistry,
          dryRun,
          out: ctx.out,
        });
        human.push(`npm: published ${data.npm.published.length}, skipped ${data.npm.skipped.length}`);
      }
      if (which === 'java' || which === 'all') {
        data.java = sdkPublish.publishMaven({
          root: ctx.root,
          repositoryId: cfg.sdk.mavenRepositoryId,
          dryRun,
          out: ctx.out,
        });
        human.push(`java: ${dryRun ? 'plan' : `published ${data.java.published.length}`}`);
      }

      return { data, human };
    },
  },
};

module.exports = { commands };
