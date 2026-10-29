'use strict';

/**
 * lib/commands.js — cms-release command implementations.
 *
 * publish / package delegate to the existing scripts (build-docker.js /
 * release.js) so release logic stays single-sourced; the CLI only infers
 * versions, normalizes arguments and runs pre-checks.
 */

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { chalk, OK, FAIL, WARN } = require('./ui');
const { normalizeVersion, compareVersions, bumpVersion } = require('./version');
const makefile = require('./makefile');
const repo = require('./repo');

const DIST_TEMPLATE_DIR = path.join('release', 'dist-template');

function die(msg) {
  console.error(`${FAIL} ${msg}`);
  return 1;
}

// Resolve the target version from -v or the latest git tag (image-tag form).
function resolveVersion(root, rawVersion) {
  if (rawVersion) {
    const ver = normalizeVersion(rawVersion);
    if (!ver) {
      console.error(`${FAIL} 无效版本号: ${rawVersion}（接受 1.2.3 / v1.2.3）`);
      return null;
    }
    return ver;
  }
  const latest = repo.latestVersion(root);
  if (!latest) {
    console.error(`${FAIL} 仓库没有 v* tag，且未指定 -v <版本>`);
    return null;
  }
  console.log(`${WARN} 未指定 -v，沿用最新 tag ${latest.tag}（镜像 tag ${latest.image}）`);
  return latest;
}

function printDiff(before, after) {
  const a = before.split('\n');
  const b = after.split('\n');
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i] === b[i]) continue;
    if (a[i] !== undefined) console.log(chalk.red(`  - ${a[i]}`));
    if (b[i] !== undefined) console.log(chalk.green(`  + ${b[i]}`));
  }
}

// Spawn `node <argv...>` inheriting stdio; honors dry-run by printing instead.
function runNode(argv, { cwd, dryRun } = {}) {
  const printable = argv.map((a) => (/\s/.test(a) ? JSON.stringify(a) : a)).join(' ');
  if (dryRun) {
    console.log(`${chalk.cyan('[dry-run]')} node ${printable}`);
    return 0;
  }
  const res = spawnSync(process.execPath, argv, { stdio: 'inherit', cwd });
  return res.status === null ? 1 : res.status;
}

// ── status ───────────────────────────────────────────────────────────────────
function cmdStatus(ctx) {
  const root = ctx.root;
  const lines = [];
  lines.push(`repo root : ${root}`);
  lines.push(`branch    : ${repo.currentBranch(root)}`);
  const dirty = repo.dirtyCount(root);
  lines.push(
    `worktree  : ${dirty < 0 ? '(git 不可用)' : dirty === 0 ? 'clean' : `${dirty} 个未提交变更`}`
  );

  const latest = repo.latestVersion(root);
  lines.push(`git tag   : ${latest ? latest.tag : '(无 v* tag)'}`);

  const mkPath = path.join(root, DIST_TEMPLATE_DIR, 'Makefile');
  if (fs.existsSync(mkPath)) {
    const mkTag = makefile.readImageTag(fs.readFileSync(mkPath, 'utf-8'));
    lines.push(`Makefile  : IMAGE_TAG ?= ${mkTag}`);
    if (latest && mkTag && mkTag !== latest.image) {
      lines.push(`${WARN} Makefile IMAGE_TAG (${mkTag}) ≠ 最新 tag (${latest.image}) — 跑: cms-release bump ${latest.image}`);
    }
  } else {
    lines.push(`${WARN} 未找到 ${mkPath}`);
  }

  const changelogPath = path.join(root, DIST_TEMPLATE_DIR, 'CHANGELOG.md');
  if (fs.existsSync(changelogPath)) {
    const head = /^##\s+(v\S+)/m.exec(fs.readFileSync(changelogPath, 'utf-8'));
    if (head) {
      lines.push(`CHANGELOG : ${head[1]}`);
      const headVer = normalizeVersion(head[1]);
      if (latest && headVer && compareVersions(headVer, latest) < 0) {
        lines.push(`${WARN} CHANGELOG 顶条 (${head[1]}) 落后最新 tag (${latest.tag}) — 记得补条目`);
      }
    }
  }

  console.log(lines.join('\n'));
  return 0;
}

// ── publish ──────────────────────────────────────────────────────────────────
function cmdPublish(ctx) {
  const { args, root } = ctx;
  const ver = resolveVersion(root, args.version);
  if (!ver) return 1;

  const script = path.join(root, 'scripts', 'build-docker.js');
  const argv = [script, ...args.positionals];
  argv.push('-v', ver.image);
  if (args.push) argv.push('--push');
  if (args.loadAndPush) argv.push('--load-and-push');
  if (args.platform) argv.push('--platform', args.platform);
  return runNode(argv, { cwd: root, dryRun: args.dryRun });
}

// ── package ──────────────────────────────────────────────────────────────────
function cmdPackage(ctx) {
  const { args, root } = ctx;
  const ver = resolveVersion(root, args.version);
  if (!ver) return 1;

  const script = path.join(root, DIST_TEMPLATE_DIR, 'scripts', 'release.js');
  if (!fs.existsSync(script)) {
    return die(`未找到 ${script} — package 需在框架仓内执行`);
  }
  const argv = [script, ver.image];
  if (args.noImages) argv.push('--no-images');
  return runNode(argv, { cwd: root, dryRun: args.dryRun });
}

// ── bump ─────────────────────────────────────────────────────────────────────
function cmdBump(ctx) {
  const { args, root } = ctx;
  const latest = repo.latestVersion(root);

  let target = null;
  if (args.positionals.length > 0) {
    target = normalizeVersion(args.positionals[0]);
    if (!target) return die(`无效版本号: ${args.positionals[0]}（接受 1.6.0 / v1.6.0）`);
  } else if (args.major || args.minor || args.patch) {
    if (!latest) return die('仓库没有 v* tag，无法自增；请显式给出版本号');
    const level = args.major ? 'major' : args.minor ? 'minor' : 'patch';
    target = bumpVersion(latest, level);
  } else {
    return die('用法: cms-release bump <1.6.0 | --patch | --minor | --major>');
  }

  if (latest && !args.force && compareVersions(target, latest) <= 0) {
    return die(`目标版本 ${target.tag} 不大于最新 tag ${latest.tag}；回退/重打请加 --force`);
  }

  const dirty = repo.dirtyCount(root);
  if (dirty > 0 && !args.force) {
    return die(`工作区有 ${dirty} 个未提交变更，先提交或加 --force`);
  }

  const mkPath = path.join(root, DIST_TEMPLATE_DIR, 'Makefile');
  if (!fs.existsSync(mkPath)) return die(`未找到 ${mkPath}`);

  const before = fs.readFileSync(mkPath, 'utf-8');
  const after = makefile.applyVersion(before, target.image);

  if (args.dryRun) {
    console.log(`${chalk.cyan('[dry-run]')} 将修改 ${mkPath}:`);
    printDiff(before, after);
    if (args.tag) {
      console.log(`${chalk.cyan('[dry-run]')} git tag -a ${target.tag} -m "release ${target.tag}"`);
    }
    return 0;
  }

  makefile.bumpMakefileFile(mkPath, target.image);
  console.log(`${OK} Makefile IMAGE_TAG → ${target.image}`);

  if (args.tag) {
    execFileSync('git', ['tag', '-a', target.tag, '-m', `release ${target.tag}`], { cwd: root });
    console.log(`${OK} git tag ${target.tag} 已创建（本地）`);
  }

  const head = /^##\s+(v\S+)/m.exec(
    fs.readFileSync(path.join(root, DIST_TEMPLATE_DIR, 'CHANGELOG.md'), 'utf-8')
  );
  console.log('\n后续手工步骤:');
  console.log(`  1. CHANGELOG.md 补 ${target.tag} 条目（顶条当前: ${head ? head[1] : '无'}）`);
  console.log(`  2. cms-release publish -v ${target.image} --load-and-push`);
  console.log(`  3. cms-release package -v ${target.image}`);
  if (args.tag) console.log(`  4. git push origin ${target.tag}`);
  return 0;
}

// ── upgrade ──────────────────────────────────────────────────────────────────
function cmdUpgrade(ctx) {
  const { args } = ctx;
  const [distDirRaw, verRaw] = args.positionals;
  if (!distDirRaw || !verRaw) {
    return die('用法: cms-release upgrade <dist-dir> <1.6.0>   # dist-dir = 解压后的分发包根目录');
  }
  const distDir = path.resolve(distDirRaw);
  const ver = normalizeVersion(verRaw);
  if (!ver) return die(`无效版本号: ${verRaw}（接受 1.6.0 / v1.6.0）`);

  const upgradeScript = path.join(distDir, 'bin', 'upgrade.js');
  if (!fs.existsSync(upgradeScript)) return die(`${upgradeScript} 不存在 — dist-dir 应是解压后的分发包根目录`);

  return runNode([upgradeScript, ver.image], { cwd: distDir, dryRun: args.dryRun });
}

const COMMANDS = {
  status: cmdStatus,
  publish: cmdPublish,
  package: cmdPackage,
  bump: cmdBump,
  upgrade: cmdUpgrade,
};

module.exports = { COMMANDS, resolveVersion, runNode, printDiff };
