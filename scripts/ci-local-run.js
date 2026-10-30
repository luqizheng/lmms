#!/usr/bin/env node
// scripts/ci-local-run.js
//
// 本地预演 GitLab CI 的阻断式 job。无需 gitlab-runner，无需 SSH 到 staging server，
// 在本机直接跑所有"能在本地复现"的 CI test stage 阻断 job + 给出 deploy/e2e:staging
// 的前置条件检查与手动复现命令。
//
// 用法：
//   pnpm run ci:local                      # 跑所有本地可复现的阻断 job（默认）
//   pnpm run ci:local -- --list            # 只列出所有 job 与状态，不执行
//   pnpm run ci:local -- --stage=test      # 只跑某 stage（test/build/e2e/deploy/publish/release/perf/cleanup）
//   pnpm run ci:local -- --job=spec_check  # 只跑某 job
//   pnpm run ci:local -- --skip-slow       # 跳过 clippy / cargo test 等慢 job
//   pnpm run ci:local -- --skip-docker     # 跳过需要 docker 的 job
//   pnpm run ci:local -- --no-color        # 关闭颜色
//
// 退出码：任一跑过的 job 失败 → 1；全过 → 0。
// 跳过 / precondition 类 job 不影响退出码（除非显式跑了）。

'use strict';

const { spawn, spawnSync } = require('node:child_process');
const { existsSync, readFileSync } = require('node:fs');
const { join } = require('node:path');
const os = require('node:os');
const { chalk, section, banner, success, warn, error, info, spinner } = require('./lib/ui');

const REPO_ROOT = require('./lib/utils').ROOT;

// ───────────────────────────────────────────────────────────────────────────
// JOB 注册表：CI job → 本地等价命令
//   category:
//     'local'        — 本机能跑，失败会阻断
//     'precondition' — 本机只检查前置条件（SSH key / staging URL / 环境变量），
//                       主体命令需在 staging server 手动跑；给出 manual repro
//     'skip'         — 本机不能跑（需 push 凭证 / tag 触发 / runner 维护）
// ───────────────────────────────────────────────────────────────────────────
const JOBS = [
  // ═══════════════════════════════════════════════════════════════════════
  // test stage —— 本地能完整复现
  // ═══════════════════════════════════════════════════════════════════════
  {
    name: 'rust_ci',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:118-147',
    description: 'Rust fmt + clippy + openapi drift + cargo test --lib',
    steps: [
      { label: 'cargo fmt --check', cmd: 'cargo', args: ['fmt', '--check', '--all'] },
      { label: 'cargo clippy --workspace -D warnings', cmd: 'cargo', args: ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'], slow: true },
      // dump-openapi 编译所有 Rust 服务的 dump_openapi bin target,首次跑 5-7 min
      // (Windows /mnt/d 跨文件系统更慢)；跳过后 git diff 也跳（无新生成 spec 可对比）
      { label: 'node scripts/dump-openapi-specs.js', cmd: 'node', args: ['scripts/dump-openapi-specs.js'], slow: true },
      { label: "git diff --exit-code -- 'services/*/openapi.yaml'", cmd: 'git', args: ['diff', '--exit-code', '--', 'services/*/openapi.yaml'], slow: true },
      { label: 'cargo test --workspace --lib', cmd: 'cargo', args: ['test', '--workspace', '--lib', '--verbose'], slow: true },
    ],
    fix: 'cargo fmt / clippy 修复后请改代码而非加 #[allow]；openapi drift 改完路由后跑 pnpm run sdk:dump',
  },
  {
    name: 'service_standard_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:149-160',
    description: '服务规范 8-item 检查',
    cmd: 'node',
    args: ['scripts/check-service-standard.js'],
    fix: 'services/<svc>/ 缺 Dockerfile / Swagger / 健康检查 / 迁移 / README / tests',
  },
  {
    name: 'spec_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:162-176',
    description: 'OpenAPI / 错误码 / 权限码校验',
    cmd: 'node',
    args: ['scripts/check-spec.js'],
    fix: '改完路由 / 错误码 / 权限码后跑 pnpm run sdk:dump 重新生成 openapi.yaml',
  },
  {
    name: 'contracts_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:178-186',
    description: '契约层 YAML → 生成代码漂移检测',
    cmd: 'node',
    args: ['scripts/contracts-gen.js', '--check'],
    fix: '改了 contracts/ 后未重新生成：pnpm run contracts:gen',
  },
  {
    name: 'doc_links_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:188-193',
    description: '文档链接死链检查',
    cmd: 'node',
    args: ['scripts/check-doc-links.js'],
  },
  {
    name: 'reconcile_todo_docs_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:195-213',
    description: 'TODO.md ↔ docs-site/* 状态行漂移检测',
    cmd: 'node',
    args: ['scripts/reconcile-todo-docs.js'],
  },
  {
    name: 'java_sdk_drift_check',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:264-276',
    description: 'sdk/java ↔ openapi 漂移检测',
    cmd: 'node',
    args: ['scripts/check-java-sdk-drift.js'],
  },
  {
    name: 'java_sdk_test',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:609-620',
    description: 'sdk/java mvn test',
    cmd: 'mvn',
    args: ['-B', 'test', '-f', 'sdk/java/pom.xml'],
  },
  {
    name: 'cargo_audit',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:797-831',
    description: 'RustSec 漏洞扫描（CI allow_failure: true）',
    cmd: 'cargo',
    args: ['audit'],
    allowFailure: true,
    fix: 'cargo install cargo-audit --locked；advisory 由 ops triage',
  },
  {
    name: 'cargo_deny',
    stage: 'test',
    category: 'local',
    ciRef: '.gitlab-ci.yml:833-863',
    description: 'cargo-deny 离线 db 校验 advisories/bans/licenses/sources',
    cmd: 'cargo',
    args: ['deny', 'check'],
    fix: 'cargo install cargo-deny --locked',
  },

  // ═══════════════════════════════════════════════════════════════════════
  // e2e stage —— 本机能跑 e2e_api/ui_test，但需要 staging URL
  // e2e:staging / e2e_migration_state_test 需要 SSH 反向隧道，本机不能跑
  // ═══════════════════════════════════════════════════════════════════════
  {
    name: 'e2e_api_test',
    stage: 'e2e',
    category: 'precondition',
    ciRef: '.gitlab-ci.yml:242-261',
    description: 'tests/e2e vitest api（指向已部署 staging）',
    cmd: 'pnpm',
    args: ['-C', 'tests/e2e', 'run', 'test:api'],
    requiresEnv: ['E2E_BASE_URL'],
    manualRepro: 'export E2E_BASE_URL=http://192.168.2.31:3000 && pnpm -C tests/e2e run test:api',
    fix: '需 staging 已部署到 192.168.2.31:3000（由 main 分支 deploy:staging 自动跑）',
  },
  {
    name: 'e2e:staging',
    stage: 'e2e',
    category: 'skip',
    ciRef: '.gitlab-ci.yml:555-607',
    description: 'SSH 反向隧道连 staging server 跑 e2e，失败时自动 rollback staging-prev',
    skipReason: '需 SSH 到 192.168.2.31 + staging-active 软链已 swap 到当前 SHA；本机无法复现',
    manualRepro: [
      '# 1) 起 SSH 反向隧道（需 staging server 上的 deploy key）',
      'ssh -i ~/.ssh/deploy_key -p 22 -o StrictHostKeyChecking=accept-new -N -f \\',
      '  -R 3000:localhost:3000 -R 8080:localhost:8080 -R 8500:localhost:8500 \\',
      '  kudi@192.168.2.31',
      '# 2) 跑 e2e（指向本地隧道端口）',
      'pnpm -C tests/e2e run test:api -- --base-url=http://localhost:8080',
      'pnpm -C tests/e2e run test:ui  -- --base-url=http://localhost:8080',
      '# 3) 若失败，手动 rollback：',
      'ssh kudi@192.168.2.31 "STAGING_BASE_DIR=/home/kudi/coder-micro-services bash /home/kudi/coder-micro-services/staging-prev/infrastructure/scripts/swap-staging-active.sh <prev-sha>"',
    ].join('\n'),
  },
  {
    name: 'e2e_migration_state_test',
    stage: 'e2e',
    category: 'skip',
    ciRef: '.gitlab-ci.yml:283-359',
    description: '7 个服务的 bootstrap token + SSH 隧道跑迁移状态校验',
    skipReason: '需 SSH 反向隧道 + 7 个 *_DEV_BT bootstrap token（CI Variables 注入），本机无法复现',
    manualRepro: '在 CI 复现或 ssh kudi@192.168.2.31 手动跑 scripts/e2e-migration-state.js',
  },

  // ═══════════════════════════════════════════════════════════════════════
  // build stage —— release:staging 本机能跑（--no-images），build_images 需要 docker
  // ═══════════════════════════════════════════════════════════════════════
  {
    name: 'release:staging',
    stage: 'build',
    category: 'local',
    ciRef: '.gitlab-ci.yml:467-489',
    description: '产 staging 部署骨架（--no-images，不打包镜像 tar）',
    cmd: 'node',
    args: ['release/dist-template/scripts/release-deploy.js', '${SHA}', '--no-images'],
    envExpand: { SHA: () => spawnSync('git', ['rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).stdout.trim() },
    fix: '检查 release/dist-template/scripts/release-deploy.js 是否能产出 MANIFEST.json + 骨架目录',
  },
  {
    name: 'build_images:staging',
    stage: 'build',
    category: 'precondition',
    ciRef: '.gitlab-ci.yml:410-464',
    description: 'docker buildx 构建 9 个服务镜像（CI 推 registry，本地用 --load）',
    cmd: 'sudo',
    args: ['-E', 'node', 'packages/cli/bin/coder.js', 'build', 'images'],
    requiresDocker: true,
    manualRepro: 'sudo -E node packages/cli/bin/coder.js build images -v $(git rev-parse --short HEAD) --load-and-push api-gateway token-service iam-service notification-service audit-service config-service file-service prue-admin-backend',
    fix: '本地构建用 --load（不推 registry）；CI 推 push 需 DOCKER_REGISTRY_USER/PASSWORD',
  },

  // ═══════════════════════════════════════════════════════════════════════
  // deploy stage —— SSH 到 staging server，本机只能检查前置条件
  // ═══════════════════════════════════════════════════════════════════════
  {
    name: 'deploy:staging',
    stage: 'deploy',
    category: 'precondition',
    ciRef: '.gitlab-ci.yml:491-553',
    description: 'SSH 到 192.168.2.31 部署 staging + atomic symlink swap + 健康 gate',
    skipReason: '需 SSH_PRIVATE_KEY + staging server 已部署 SSH key；本机只能检查前置条件',
    preconditionChecks: [
      { name: '~/.ssh/deploy_key 存在', check: () => existsSync(join(os.homedir(), '.ssh', 'deploy_key')) },
      { name: 'staging server 192.168.2.31:8080 可达', check: async () => await checkUrl('http://192.168.2.31:8080') },
      { name: 'staging server 192.168.2.31:3000/health 返回 ok', check: async () => (await checkUrl('http://192.168.2.31:3000/health'))?.body === '{"status":"ok"}' },
    ],
    manualRepro: [
      '# deploy:staging 是 5 步串行 SSH，本机不能整体复现，可逐段手动跑：',
      'SHA=$(git rev-parse --short HEAD)',
      'DIST_DIR=release/dist-template/release/coder-micro-services-${SHA}-deploy',
      '# 1) tar over ssh 传骨架',
      'tar -C "$DIST_DIR" -cf - . | ssh -i ~/.ssh/deploy_key kudi@192.168.2.31 "tar -C /home/kudi/coder-micro-services/staging-${SHA} -xf -"',
      '# 2) 远端拉镜像',
      'ssh kudi@192.168.2.31 "cd /home/kudi/coder-micro-services/staging-${SHA} && IMAGE_TAG=${SHA} REGISTRY=zhcoder-docker-registry.com:8000 BASE_REGISTRY=zhcoder-docker-registry.com:8000/official node bin/load-images.js --mode=pull"',
      '# 3) atomic swap staging-active',
      'ssh kudi@192.168.2.31 "STAGING_BASE_DIR=/home/kudi/coder-micro-services bash /home/kudi/coder-micro-services/staging-${SHA}/infrastructure/scripts/swap-staging-active.sh ${SHA}"',
      '# 4) 健康 gate（轮询 300s）',
      'for i in $(seq 1 30); do curl -fsS --max-time 10 http://192.168.2.31:3000/health && break || sleep 10; done',
    ].join('\n'),
    fix: '本机无法复现 SSH 部署；常见失败：staging server 磁盘满、镜像 tag 不对、staging-active 软链权限',
  },

  // ═══════════════════════════════════════════════════════════════════════
  // publish / release / perf / cleanup —— 需凭证 / tag / staging 隧道，跳过
  // ═══════════════════════════════════════════════════════════════════════
  {
    name: 'release:dist',
    stage: 'release',
    category: 'local',
    ciRef: '.gitlab-ci.yml:768-792',
    description: '打离线分发包（tag 触发，本地用 local-test 版本号预演）',
    cmd: 'node',
    args: ['release/dist-template/scripts/release.js', 'local-test', '--no-images'],
    fix: '本地预演用 --no-images；真发布需前置 build_images:tagged 完成',
  },
  {
    name: 'sdk_publish',
    stage: 'publish', category: 'skip', ciRef: '.gitlab-ci.yml:622-642',
    description: 'Java SDK 发布到私有 Maven registry',
    skipReason: '需 MAVEN_USERNAME/MAVEN_PASSWORD + tag 触发',
    manualRepro: 'node packages/cli/bin/coder.js release publish maven --version <VERSION> --dry-run',
  },
  {
    name: 'rust_publish',
    stage: 'publish', category: 'skip', ciRef: '.gitlab-ci.yml:649-664',
    description: 'Rust libs + SDK 发布到 kellnr',
    skipReason: '需 KELLNR_TOKEN + tag 触发',
    manualRepro: 'node packages/cli/bin/coder.js release publish rust <VERSION> --registry kellnr --dry-run',
  },
  {
    name: 'build_images:tagged',
    stage: 'publish', category: 'skip', ciRef: '.gitlab-ci.yml:670-726',
    description: 'tag 触发，按版本号构建推送 9 个服务镜像',
    skipReason: '需 DOCKER_REGISTRY_USER/PASSWORD + tag 触发',
    manualRepro: 'sudo -E node packages/cli/bin/coder.js build images -v <VERSION> --push',
  },
  {
    name: 'publish_packages',
    stage: 'publish', category: 'skip', ciRef: '.gitlab-ci.yml',
    description: '@digua/contracts + @digua/http + @digua/cli 按 tag 版本发布到 npm.kudi.repos',
    skipReason: '需 NPM_TOKEN/NPM_USERNAME + tag 触发',
    manualRepro: 'node packages/cli/bin/coder.js release publish npm --version <next-version> --dry-run',
  },
  {
    name: 'k6_baseline',
    stage: 'perf', category: 'skip', ciRef: '.gitlab-ci.yml:869-916',
    description: 'k6 性能基线（需 staging 隧道）',
    skipReason: '需 SSH 反向隧道 + k6 镜像',
    manualRepro: 'docker run --rm grafana/k6:0.57.0 run tests/load/s2-*.js',
  },
  {
    name: 'cleanup:buildx',
    stage: 'cleanup', category: 'skip', ciRef: '.gitlab-ci.yml:79-105',
    description: '清理 GitLab shell runner 主机的 buildx 缓存',
    skipReason: 'runner 主机维护任务，本地无需',
  },
];

// ───────────────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────────────

async function checkUrl(url) {
  return new Promise((resolve) => {
    const lib = url.startsWith('https') ? require('node:https') : require('node:http');
    const req = lib.get(url, { timeout: 5000 }, (res) => {
      let body = '';
      res.on('data', (c) => (body += c));
      res.on('end', () => resolve({ ok: true, status: res.statusCode, body }));
    });
    req.on('error', () => resolve(null));
    req.on('timeout', () => { req.destroy(); resolve(null); });
  });
}

function hasBin(cmd) {
  const isWin = process.platform === 'win32';
  const probe = isWin ? 'where' : 'which';
  try {
    spawnSync(probe, [cmd], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

function runStep(cmd, args, opts = {}) {
  // ${VAR} 展开（仅支持 envExpand 配置的字段，避免引号嵌套坑）
  const expanded = args.map((a) => {
    const m = String(a).match(/^\$\{(\w+)\}$/);
    if (m && opts.envExpand && opts.envExpand[m[1]]) {
      return opts.envExpand[m[1]]();
    }
    return a;
  });
  const isWin = process.platform === 'win32';
  // Windows 上只有 .cmd 包装器（pnpm/npx/mvn）需要 shell:true 才能从 PATH 找到。
  // node / cargo / git 这些是 .exe，直接 spawn 即可，避免 Node 22 的
  // "shell option can lead to security vulnerabilities" DeprecationWarning。
  const cmdWrapOnWindows = ['pnpm', 'npx', 'mvn', 'pnpm.cmd', 'npx.cmd', 'mvn.cmd'];
  const useShell = isWin && cmdWrapOnWindows.includes(cmd);
  const realCmd = useShell && isWin ? process.env.ComSpec || 'cmd.exe' : cmd;
  const realArgs = useShell && isWin ? ['/c', cmd, ...expanded] : expanded;
  const t0 = Date.now();
  const result = spawnSync(realCmd, realArgs, {
    cwd: REPO_ROOT,
    encoding: 'utf8',
    stdio: 'inherit',
    shell: useShell,
    env: process.env,
  });
  const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
  return { status: result.status, signal: result.signal, elapsed };
}

// ───────────────────────────────────────────────────────────────────────────
// 主流程：parse args → filter jobs → run → summary
// ───────────────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const out = { list: false, stage: null, job: null, skipSlow: false, skipDocker: false, noColor: false };
  for (const a of argv) {
    if (a === '--list') out.list = true;
    else if (a.startsWith('--stage=')) out.stage = a.slice(8);
    else if (a.startsWith('--job=')) out.job = a.slice(6);
    else if (a === '--skip-slow') out.skipSlow = true;
    else if (a === '--skip-docker') out.skipDocker = true;
    else if (a === '--no-color') { out.noColor = true; process.env.FORCE_COLOR = '0'; }
    else if (a === '-h' || a === '--help') {
      printHelp();
      process.exit(0);
    } else {
      error(`未知参数: ${a}`);
      process.exit(2);
    }
  }
  return out;
}

function printHelp() {
  console.log(`
${chalk.bold('ci:local')} — 本地预演 GitLab CI 阻断 job

用法:
  pnpm run ci:local [flags]

Flags:
  --list              只列出所有 job 与状态，不执行
  --stage=<name>      只跑某 stage (test/build/e2e/deploy/publish/release/perf/cleanup)
  --job=<name>        只跑某 job（如 --job=spec_check）
  --skip-slow         跳过 clippy / cargo test 等慢 step
  --skip-docker       跳过需要 docker 的 job
  --no-color          关闭颜色
  -h, --help          显示此帮助

退出码：任一跑过的 job 失败 → 1；全过 → 0
`);
}

function listJobs() {
  banner('CI Jobs 本地复现能力一览');
  const stages = [...new Set(JOBS.map((j) => j.stage))];
  for (const st of stages) {
    section(`stage: ${st}`);
    for (const j of JOBS.filter((j) => j.stage === st)) {
      const cat = j.category === 'local' ? chalk.green('  ✓ local')
        : j.category === 'precondition' ? chalk.yellow('  ⚠ precondition')
        : chalk.gray('  ✗ skip');
      console.log(`  ${cat}  ${chalk.bold(j.name)}  ${chalk.gray(j.ciRef)}`);
      console.log(`           ${j.description || ''}`);
    }
  }
  console.log('\n说明：');
  console.log('  ✓ local        本机能完整复现，失败会阻断');
  console.log('  ⚠ precondition 本机只检查前置条件，主体需在 staging server 手动跑');
  console.log('  ✗ skip         需凭证 / tag / SSH，本机不能跑（脚本会给出 manual repro 命令）');
}

async function runJob(job, opts) {
  banner(`${job.name}  ${chalk.gray(job.ciRef)}`);
  info(job.description || '');

  // skip 类：打印手动复现命令后返回
  if (job.category === 'skip') {
    warn(`跳过：${job.skipReason}`);
    if (job.manualRepro) {
      console.log(chalk.gray('\n手动复现命令：'));
      console.log(chalk.gray(job.manualRepro));
    }
    return { status: 'skip' };
  }

  // precondition 类：先跑前置检查，再决定是否跑主体
  if (job.category === 'precondition') {
    if (job.requiresEnv && !job.requiresEnv.every((v) => process.env[v])) {
      warn(`跳过：缺少环境变量 ${job.requiresEnv.join(', ')}`);
      if (job.manualRepro) {
        console.log(chalk.gray('\n手动复现命令：'));
        console.log(chalk.gray(job.manualRepro));
      }
      return { status: 'skip' };
    }
    if (job.requiresDocker && !hasBin('docker')) {
      warn('跳过：未检测到 docker CLI');
      if (job.manualRepro) console.log(chalk.gray('\n手动复现：\n' + job.manualRepro));
      return { status: 'skip' };
    }
    if (job.preconditionChecks) {
      for (const c of job.preconditionChecks) {
        const sp = spinner(c.name).start();
        try {
          const ok = await c.check();
          ok ? sp.succeed(c.name) : sp.fail(c.name);
        } catch (e) {
          sp.fail(`${c.name} (${e.message})`);
        }
      }
      // precondition 类本机不能整体复现，无论检查通过与否都给 manual repro 命令
      if (job.manualRepro) {
        info('主体命令需在 staging server 手动跑：');
        console.log(chalk.gray(job.manualRepro));
      }
      return { status: 'precondition' };
    }
    // 没有前置检查的 precondition（e2e_api/ui_test）：直接跑主体
  }

  // local 类：跑主体
  if (job.cmd) {
    // 单命令 job
    const step = { label: `${job.cmd} ${job.args.join(' ')}`, cmd: job.cmd, args: job.args, slow: false };
    if (opts.skipSlow && step.slow) {
      warn(`跳过慢 step：${step.label}`);
      return { status: 'skip' };
    }
    const r = runStep(job.cmd, job.args, { envExpand: job.envExpand });
    if (r.status === 0) {
      success(`${job.name}  (${r.elapsed}s)`);
      return { status: 'pass', elapsed: r.elapsed };
    } else {
      error(`${job.name} 失败 (exit ${r.status}${r.signal ? ' / ' + r.signal : ''})`);
      console.log(chalk.gray(`CI 等价位置：${job.ciRef}`));
      if (job.fix) console.log(chalk.gray(`修复建议：${job.fix}`));
      return { status: job.allowFailure ? 'warn' : 'fail', elapsed: r.elapsed };
    }
  } else if (job.steps) {
    // 多 step job（rust_ci）
    let anyFail = false;
    let totalElapsed = 0;
    for (const step of job.steps) {
      if (opts.skipSlow && step.slow) {
        warn(`跳过慢 step：${step.label}`);
        continue;
      }
      const sp = spinner(step.label).start();
      const r = runStep(step.cmd, step.args, { envExpand: job.envExpand });
      totalElapsed += parseFloat(r.elapsed);
      if (r.status === 0) {
        sp.succeed(`${step.label}  (${r.elapsed}s)`);
      } else {
        sp.fail(`${step.label}  (exit ${r.status})`);
        anyFail = true;
      }
    }
    if (anyFail) {
      error(`${job.name} 失败`);
      console.log(chalk.gray(`CI 等价位置：${job.ciRef}`));
      if (job.fix) console.log(chalk.gray(`修复建议：${job.fix}`));
      return { status: job.allowFailure ? 'warn' : 'fail', elapsed: totalElapsed.toFixed(1) };
    }
    success(`${job.name}  (${totalElapsed.toFixed(1)}s)`);
    return { status: 'pass', elapsed: totalElapsed.toFixed(1) };
  }
  return { status: 'skip' };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));

  if (opts.list) {
    listJobs();
    return;
  }

  let selected = JOBS;
  if (opts.stage) selected = selected.filter((j) => j.stage === opts.stage);
  if (opts.job) {
    selected = selected.filter((j) => j.name === opts.job);
    if (!selected.length) {
      error(`未找到 job: ${opts.job}（用 --list 查看所有 job 名）`);
      process.exit(2);
    }
  }
  if (opts.skipDocker) selected = selected.filter((j) => !j.requiresDocker);

  banner(`本地预演 CI —— 共 ${selected.length} 个 job`);
  info(`仓库根：${REPO_ROOT}`);
  info(`平台：${process.platform} ${os.release()}`);

  const results = [];
  for (const job of selected) {
    const r = await runJob(job, opts);
    results.push({ name: job.name, stage: job.stage, category: job.category, ...r });
  }

  // 总结表
  banner('结果总结');
  const pad = Math.max(...results.map((r) => r.name.length));
  let failCount = 0;
  for (const r of results) {
    const sym = r.status === 'pass' ? chalk.green('✓')
      : r.status === 'fail' ? chalk.red('✗')
      : r.status === 'warn' ? chalk.yellow('⚠')
      : chalk.gray('·');
    const tail = r.elapsed ? chalk.gray(` ${r.elapsed}s`) : '';
    console.log(`  ${sym} ${r.name.padEnd(pad)}  ${chalk.gray(r.stage.padEnd(8))}${tail}`);
    if (r.status === 'fail') failCount++;
  }

  console.log('');
  if (failCount > 0) {
    error(`${failCount} 个 job 失败`);
    process.exit(1);
  } else {
    success('所有本地可复现的 CI job 通过');
  }
}

main().catch((e) => {
  error(`ci:local 异常：${e.stack || e.message}`);
  process.exit(1);
});
