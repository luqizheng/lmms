#!/usr/bin/env node
/**
 * scripts/build-docker.js
 *
 * 批量构建 micro-server 镜像。支持：
 *   - 交互式 / 参数式 两种模式（无参数 = 交互式）
 *   - 版本号 tag (-v 1.0.0) -> 同时打 :<ver> 与 :latest
 *   - 多 CPU 架构 (--platform linux/amd64,linux/arm64)
 *   - 可选推送到 zhcoder-docker-registry.com:8000/micro-server
 *     --push           直接 buildx --push（不留本地）
 *     --load-and-push  单架构: --load 后再 docker push 每 tag（本地留镜像 + 推送）
 *                      多架构: 回退 --push（多架构无法 load），提示
 *
 * 用法：
 *   node scripts/build-docker.js                                # 交互式
 *   node scripts/build-docker.js -v 1.0.0 --push                # 参数式全量
 *   node scripts/build-docker.js --load-and-push -v 1.0.0       # 本地留镜像+推送
 *   node scripts/build-docker.js api-gateway token-service -v 1.0.0 --push
 *   node scripts/build-docker.js --platform linux/amd64,linux/arm64 --push
 *   node scripts/build-docker.js prune                          # 清理 buildx cache
 *   pnpm run docker:build -- -v 1.0.0 --load-and-push           # npm script 传参
 *
 * 多架构注意：buildx 多架构镜像无法 --load 到本地 docker，必须 --push。
 *   若 --platform 多架构且未 --push/--load-and-push，自动开启 --push 并提示。
 *   --load-and-push 在多架构下回退为 --push。
 *
 * 非 TTY 环境（CI，stdin 非 TTY）：无参数时回退到参数式默认值
 *   (无版本号 / linux/amd64 / 不推送 / 全部服务)。
 */

const fs = require('node:fs');
const path = require('node:path');
const prompts = require('prompts');
const { spawnSync } = require('node:child_process');
const { chalk } = require('./lib/ui.js');

const ROOT_DIR = path.join(__dirname, '..');
const SERVICES_DIR = path.join(ROOT_DIR, 'services');

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

// pushMode: 'none' | 'push' | 'loadAndPush'
const PUSH_NONE = 'none';
const PUSH_PUSH = 'push';
const PUSH_LOAD_AND_PUSH = 'loadAndPush';

const OK = chalk.green('\u2713');
const FAIL = chalk.red('\u2717');
const WARN = chalk.yellow('!');

function onCancel() {
  console.error(chalk.yellow('\n✗ 操作被取消'));
  process.exit(130);
}

// ---------- arg parsing ----------

function parseArgs(argv) {
  const opts = {
    version: null,
    push: false,
    loadAndPush: false,
    platforms: null,
    registry: DEFAULT_REGISTRY,
    repo: DEFAULT_REPO,
    services: [],
    help: false,
    interactive: false,
    unknown: [],
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '-h': case '--help': opts.help = true; break;
      case '-v': case '--version': opts.version = argv[++i] || null; break;
      case '--push': opts.push = true; opts.loadAndPush = false; break;
      case '--no-push': opts.push = false; opts.loadAndPush = false; break;
      case '--load-and-push': case '--load-push':
        opts.loadAndPush = true; opts.push = false; break;
      case '-p': case '--platform': opts.platforms = argv[++i] || null; break;
      case '--registry': opts.registry = argv[++i] || DEFAULT_REGISTRY; break;
      case '--repo': opts.repo = argv[++i] || DEFAULT_REPO; break;
      case '-i': case '--interactive': opts.interactive = true; break;
      default:
        if (a.startsWith('-')) {
          opts.unknown.push(a);
        } else if (SERVICES.includes(a)) {
          opts.services.push(a);
        } else {
          opts.unknown.push(a);
        }
    }
  }
  return opts;
}

function printHelp() {
  console.log(`usage: node scripts/build-docker.js [services...] [options]
       node scripts/build-docker.js [prune|clean]

批量构建 micro-server 镜像。

模式：
  无参数                交互式（TTY），非 TTY 回退到参数式默认值
  -i, --interactive     强制交互式
  [services...] [opts]  参数式

选项：
  -v, --version <ver>   版本号 tag (如 1.0.0)；同时打 :<ver> 与 :latest
  --push                buildx --push 直接推送 (不留本地镜像)
  --load-and-push       单架构: --load 后再 docker push 每 tag (本地留镜像+推送)
                        多架构: 回退 --push (多架构无法 load)，提示
  --no-push             不推送 (参数式默认)
  -p, --platform <list> CPU 架构, 逗号分隔 (默认 ${DEFAULT_PLATFORM})
                        多架构示例: linux/amd64,linux/arm64
  --registry <url>      覆盖 registry (默认 ${DEFAULT_REGISTRY})
  --repo <name>         覆盖 repo (默认 ${DEFAULT_REPO})
  -i, --interactive     强制交互式
  -h, --help            显示帮助

服务列表：${SERVICES.join(', ')}

示例：
  node scripts/build-docker.js
  node scripts/build-docker.js -v 1.0.0 --push
  node scripts/build-docker.js --load-and-push -v 1.0.0
  node scripts/build-docker.js api-gateway token-service -v 1.0.0 --push
  node scripts/build-docker.js --platform linux/amd64,linux/arm64 --push
  pnpm run docker:build -- -v 1.0.0 --load-and-push

多架构注意：buildx 多架构镜像无法 --load 到本地，必须 --push。
  若 --platform 多架构且未 --push/--load-and-push，自动开启 --push 并提示。
  --load-and-push 在多架构下回退为 --push。
`);
}

// ---------- small helpers ----------

function validateVersion(v) {
  if (!v) return true;
  return /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(v);
}

function parsePlatforms(s) {
  if (!s) return null;
  const list = s.split(',').map(x => x.trim()).filter(Boolean);
  if (list.length === 0) return null;
  for (const p of list) {
    if (!/^linux\//.test(p)) {
      throw new Error(`无效平台 "${p}"，需形如 linux/amd64`);
    }
  }
  return list;
}

function shouldProceed(ans) {
  // 空串=默认 yes；y/yes=yes；其余=no
  return ans === '' || /^[yY]/.test(ans);
}

function ask(_rl, question, defaultValue) {
  const hint = defaultValue !== undefined ? ` [${defaultValue}]` : '';
  return prompts({ type: 'text', name: 'v', message: `${question}${hint}` }, { onCancel })
    .then((r) => (r.v || '').trim());
}

function pushLabel(mode) {
  if (mode === PUSH_PUSH) return chalk.green('push');
  if (mode === PUSH_LOAD_AND_PUSH) return chalk.green('load+push');
  return chalk.dim('no');
}

/**
 * 规整 pushMode：多架构下 'none' -> 'push'（自动开启），'loadAndPush' -> 'push'（回退）。
 */
function normalizePushMode(rawMode, multiArch) {
  if (!multiArch) return rawMode;
  if (rawMode === PUSH_NONE) {
    console.log(`${WARN} [info] 多架构无法 --load，自动开启 --push。`);
    return PUSH_PUSH;
  }
  if (rawMode === PUSH_LOAD_AND_PUSH) {
    console.log(`${WARN} [info] 多架构下 --load-and-push 回退为 --push（无法 load）。`);
    return PUSH_PUSH;
  }
  return PUSH_PUSH;
}

// ---------- interactive ----------

async function interactivePrompt() {
  const isTTY = process.stdin.isTTY === true;
  if (!isTTY) {
    console.log(`${WARN} [warn] stdin 非 TTY，回退到参数式默认值：`);
    console.log(`        version=latest, platform=${DEFAULT_PLATFORM}, push=none, services=all`);
    return {
      version: null,
      platforms: [DEFAULT_PLATFORM],
      pushMode: PUSH_NONE,
      registry: DEFAULT_REGISTRY,
      repo: DEFAULT_REPO,
      services: SERVICES.slice(),
    };
  }

  const rl = null;
  console.log('\n' + chalk.bold.cyan('=== docker build 交互式 ===') + '\n');

  // 1. version
  let version = await ask(rl, '版本号 tag (如 1.0.0)', '跳过');
  while (version !== '' && version !== '跳过' && !validateVersion(version)) {
    console.log(`${FAIL} 版本号只能含字母数字 . _ -`);
    version = await ask(rl, '版本号 tag', '跳过');
  }
  if (version === '' || version === '跳过') version = '';

  // 2. platforms
  let platInput = await ask(rl, 'CPU 架构 (逗号分隔)', DEFAULT_PLATFORM);
  if (platInput === '') platInput = DEFAULT_PLATFORM;
  let platforms;
  try {
    platforms = parsePlatforms(platInput) || [DEFAULT_PLATFORM];
  } catch (e) {
    console.log(`${FAIL} ${e.message}，使用默认 ${DEFAULT_PLATFORM}`);
    platforms = [DEFAULT_PLATFORM];
  }

  // 3. push mode
  const multiArch = platforms.length > 1;
  let pushMode;
  if (multiArch) {
    console.log(`${WARN} [info] 多架构必须推送到 registry，push 模式固定为 push。`);
    pushMode = PUSH_PUSH;
  } else {
    const pushAns = await ask(rl, '推送? (y=推送 / n=不推 / l=load并推送)', 'y');
    const lc = pushAns.toLowerCase();
    if (pushAns === '' || lc === 'y' || lc === 'yes') pushMode = PUSH_PUSH;
    else if (lc === 'l' || lc === 'load' || lc === 'lp') pushMode = PUSH_LOAD_AND_PUSH;
    else pushMode = PUSH_NONE;
  }

  // 4. services
  const svcInput = await ask(rl, '构建哪些服务 (逗号分隔)', '全部');
  let services;
  if (svcInput === '' || svcInput === '全部') {
    services = SERVICES.slice();
  } else {
    services = svcInput.split(',').map(s => s.trim()).filter(s => SERVICES.includes(s));
    if (services.length === 0) {
      console.log(`${FAIL} 无有效服务，使用全部`);
      services = SERVICES.slice();
    }
  }

  // 5. confirm
  console.log(`\n${chalk.bold(`--- 确认 ---`)}`);
  console.log(`  version : ${version || '(latest only)'}`);
  console.log(`  platform: ${platforms.join(',')}`);
  console.log(`  push    : ${pushLabel(pushMode)}`);
  console.log(`  registry: ${DEFAULT_REGISTRY}/${DEFAULT_REPO}`);
  console.log(`  services: ${services.join(', ')} (${services.length})`);

  const go = await ask(rl, '\n开始构建? (y/n)', 'y');
  void rl;

  if (!shouldProceed(go)) {
    console.log(`${chalk.yellow(`已取消。`)}`);
    process.exit(0);
  }

  return { version: version || null, platforms, pushMode, registry: DEFAULT_REGISTRY, repo: DEFAULT_REPO, services };
}

// ---------- build ----------

function buildService(name, ctx) {
  const { version, platforms, pushMode, registry, repo } = ctx;
  const multiArch = platforms.length > 1;

  let dockerfile, cwd, relDf;
  if (name === 'docs-site') {
    // docs-site 的 vitepress 依赖在仓库根 package.json，构建上下文必须是 ROOT_DIR
    dockerfile = path.join(ROOT_DIR, 'docs-site', 'Dockerfile');
    cwd = ROOT_DIR;
    relDf = 'docs-site/Dockerfile';
  } else if (name === 'prue-admin-backend') {
    // prue-admin-backend 镜像名，源码目录仍是 prue_backend
    dockerfile = path.join(SERVICES_DIR, 'prue_backend', 'Dockerfile');
    cwd = ROOT_DIR;
    relDf = 'services/prue_backend/Dockerfile';
  } else {
    dockerfile = path.join(SERVICES_DIR, name, 'Dockerfile');
    cwd = ROOT_DIR;
    relDf = path.relative(ROOT_DIR, dockerfile);
  }
  if (!fs.existsSync(dockerfile)) {
    console.log(`${FAIL} [skip] ${name} 无 Dockerfile`);
    return false;
  }

  // docs-site 的 Dockerfile 只打包预构建产物，需先在宿主机跑 vitepress build
  // （vitepress lastUpdated 依赖完整 git 仓库，容器内无法获取真实最后更新时间）
  if (name === 'docs-site') {
    console.log(`${chalk.dim(`[pre-build]`)} pnpm docs:build (生成静态站点)`);
    const r = spawnSync('pnpm', ['docs:build'], { cwd: ROOT_DIR, stdio: 'inherit', shell: false });
    if (r.error) {
      console.log(`${FAIL} [pre-build-fail] docs-site: 无法执行 pnpm: ${r.error.message}`);
      return false;
    }
    if (r.status !== 0) {
      console.log(`${FAIL} [pre-build-fail] docs-site: pnpm docs:build 失败 (exit ${r.status})`);
      return false;
    }
  }

  const baseRef = `${registry}/${repo}/${name}`;
  const tags = [version ? `${baseRef}:${version}` : null, `${baseRef}:latest`].filter(Boolean);
  const uniqTags = [...new Set(tags)];   // version=latest 时去重

  const usePushFlag = pushMode === PUSH_PUSH || multiArch;  // multiArch 已规整为 push
  const buildCmd = ['docker', 'buildx', 'build'];
  buildCmd.push(usePushFlag ? '--push' : '--load');
  buildCmd.push('--platform', platforms.join(','));
  buildCmd.push('-f', relDf);
  for (const t of uniqTags) buildCmd.push('-t', t);
  buildCmd.push('.');

  console.log(`\n${chalk.bold(`[build]`)} ${name} -> ${uniqTags.join(', ')}`);
  console.log(`${chalk.dim(`$ ${buildCmd.join(' ')}`)}`);
  const res = spawnSync(buildCmd[0], buildCmd.slice(1), { cwd, stdio: 'inherit', shell: false });

  if (res.status !== 0) {
    console.log(`${FAIL} [fail] ${name}`);
    return false;
  }

  // loadAndPush: 单架构 --load 成功后，逐 tag docker push
  if (pushMode === PUSH_LOAD_AND_PUSH && !multiArch) {
    for (const t of uniqTags) {
      console.log(`${chalk.dim(`[push]`)} ${t}`);
      const r = spawnSync('docker', ['push', t], { stdio: 'inherit', shell: false });
      if (r.status !== 0) {
        console.log(`${FAIL} [push-fail] ${t}`);
        return false;
      }
    }
  }

  console.log(`${OK} [ok]   ${name}`);
  return true;
}

function pruneCache() {
  console.log(`${WARN} [clean] 清理 buildx cache (cargo-registry / cargo-git)`);
  for (const id of ['cargo-registry', 'cargo-git']) {
    const res = spawnSync('docker', ['buildx', 'prune', '--force', '--cache-filter', `id==${id}`], {
      stdio: 'inherit', shell: false,
    });
    if (res.status !== 0) {
      console.log(`${FAIL} [warn] prune ${id} 失败`);
    }
  }
}

// ---------- main ----------

async function main() {
  const argv = process.argv.slice(2);

  if (argv[0] === 'clean' || argv[0] === 'prune') {
    pruneCache();
    return;
  }

  const opts = parseArgs(argv);

  if (opts.help) {
    printHelp();
    return;
  }

  if (opts.unknown.length > 0) {
    console.log(`${WARN} [warn] 忽略未知参数: ${opts.unknown.join(', ')}`);
  }

  const noArgs = argv.length === 0;
  let ctx;
  if (noArgs || opts.interactive) {
    ctx = await interactivePrompt();
  } else {
    let platforms;
    try {
      platforms = opts.platforms ? parsePlatforms(opts.platforms) : [DEFAULT_PLATFORM];
      if (!platforms) platforms = [DEFAULT_PLATFORM];
    } catch (e) {
      console.log(`${FAIL} ${e.message}`);
      process.exit(2);
    }
    if (!validateVersion(opts.version)) {
      console.log(`${FAIL} 无效版本号: ${opts.version}`);
      process.exit(2);
    }
    const multiArch = platforms.length > 1;
    let rawMode;
    if (opts.loadAndPush) rawMode = PUSH_LOAD_AND_PUSH;
    else if (opts.push) rawMode = PUSH_PUSH;
    else rawMode = PUSH_NONE;
    const pushMode = normalizePushMode(rawMode, multiArch);
    const services = opts.services.length > 0 ? opts.services : SERVICES.slice();
    ctx = {
      version: opts.version,
      platforms,
      pushMode,
      registry: opts.registry,
      repo: opts.repo,
      services,
    };
  }

  if (ctx.services.length === 0) {
    console.log(`${FAIL} 无可用服务。有效值：${SERVICES.join(', ')}`);
    process.exit(2);
  }

  console.log(`\n${chalk.bold(`=== build plan ===`)}`);
  console.log(`  version : ${ctx.version || '(latest only)'}`);
  console.log(`  platform: ${ctx.platforms.join(',')}`);
  console.log(`  push    : ${pushLabel(ctx.pushMode)}`);
  console.log(`  registry: ${ctx.registry}/${ctx.repo}`);
  console.log(`  services: ${ctx.services.join(', ')} (${ctx.services.length})`);
  if (ctx.platforms.length > 1) {
    console.log(`${chalk.dim(`  hint    : 若 arm64 失败，启用 qemu: docker run --privileged --rm tonistiigi/binfmt --install all`)}`);
  }

  const start = Date.now();
  let ok = 0, fail = 0;
  for (const name of ctx.services) {
    if (buildService(name, ctx)) ok += 1; else fail += 1;
  }
  const sec = ((Date.now() - start) / 1000).toFixed(1);
  console.log(`\n${chalk.bold(`[done]`)} 成功 ${chalk.green(`${ok}`)} 失败 ${chalk.red(`${fail}`)} 耗时 ${sec}s`);
  if (fail > 0) process.exit(1);
}

main().catch((err) => {
  console.error(`${FAIL} build-docker crashed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
