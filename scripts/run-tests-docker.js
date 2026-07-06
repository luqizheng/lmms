// 在 docker 容器中跑各 service 的 cargo 测试（volume mapping 模型）。
// 源码、target、cargo registry 全部用挂载，避免 rebuild 镜像，增量编译跨 service 复用。
//
// 两阶段：
//   1. compile：`cargo nextest run --no-run -p <svc>` 并发 4（编译吃多核）
//   2. run    ：`cargo nextest run -p <svc>` 并发 2（DB 串行）
//
// 用法：
//   node scripts/run-tests-docker.js                       # 跑全部 services（共享 mysql）
//   node scripts/run-tests-docker.js iam-service           # 跑指定 service（可多个）
//   node scripts/run-tests-docker.js --no-shared-mysql     # 关闭共享 mysql（回归 / 调试）
//   node scripts/run-tests-docker.js --incremental         # 只跑改动文件涉及的 service
//   node scripts/run-tests-docker.js iam-service --incremental   # 单 service 也可叠加
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');

const IMAGE = 'zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl';
const TARGET = 'x86_64-unknown-linux-musl';

// 每个 service 源码目录前缀；增量模式按这个匹配 git 改动文件。
const SERVICE_DIRS = {
  'api-gateway': 'services/api-gateway',
  'token-service': 'services/token-service',
  'iam-service': 'services/iam-service',
  'notification-service': 'services/notification-service',
  'audit-service': 'services/audit-service',
  'config-service': 'services/config-service',
  'file-service': 'services/file-service',
  'prue_backend': 'services/prue_backend',
};
const SERVICES = Object.keys(SERVICE_DIRS);

const CONCURRENCY_COMPILE = 4;
const CONCURRENCY_RUN = 2;

const MYSQL_CONTAINER_NAME = 'coder-test-mysql';
const MYSQL_PORT = 33306;
const MYSQL_IMAGE = 'mysql:8.0';
// cargo-nextest 二进制 bind mount 到这个 volume，下次复用。
const NEXTEST_BIN_VOLUME = 'coder-cargo-nextest-bin';

function pickTargets(args) {
  const filtered = args.filter(s => SERVICES.includes(s));
  return filtered.length > 0 ? filtered : SERVICES;
}

function pickSharedMysql(args) {
  return !args.includes('--no-shared-mysql');
}

// 顶层 Cargo.toml / Cargo.lock / libs/ / scripts/ 改动 → 全部 service。
const GLOBAL_PREFIXES = ['Cargo.toml', 'Cargo.lock', 'libs/', 'scripts/'];

function pickIncrementalTargets(args) {
  if (!args.includes('--incremental')) return null;

  const status = spawnSync('git', ['status', '--porcelain', '--untracked-files=all'], { encoding: 'utf8', cwd: ROOT });
  if (status.status !== 0) return null;

  const changedFiles = [];
  for (const line of (status.stdout || '').split('\n')) {
    if (!line) continue;
    const m = line.match(/^[ ?!]{2}\s+(.+)$/);
    if (!m) continue;
    const f = m[1].includes(' -> ') ? m[1].split(' -> ').pop() : m[1];
    changedFiles.push(f);
  }

  if (changedFiles.length === 0) {
    console.log('[incremental] no changes detected, nothing to test');
    return [];
  }

  const globalHit = changedFiles.some(f => GLOBAL_PREFIXES.some(p => f === p.slice(0, -1) || f.startsWith(p)));
  if (globalHit) {
    console.log('[incremental] global change detected, running all services');
    return SERVICES.slice();
  }

  const targets = [];
  for (const [svc, dir] of Object.entries(SERVICE_DIRS)) {
    if (changedFiles.some(f => f === dir || f.startsWith(dir + '/'))) targets.push(svc);
  }
  if (targets.length === 0) {
    console.log('[incremental] no service touched, nothing to test');
    return [];
  }
  console.log('[incremental] changed services:', targets.join(', '));
  return targets;
}

function ensureNextest() {
  // 用独立 volume 持久化 cargo-nextest 二进制，避免每次重装。
  const check = spawnSync('docker', ['run', '--rm',
    '-v', `${NEXTEST_BIN_VOLUME}:/cargo-bin`,
    IMAGE,
    'sh', '-c', 'test -x /cargo-bin/cargo-nextest && echo OK'],
  { encoding: 'utf8' });
  if (check.status === 0 && /OK/.test(check.stdout || '')) return;

  console.log('[nextest] installing cargo-nextest (first run, ~2 min)...');
  const install = spawnSync('docker', ['run', '--rm',
    '-v', `${ROOT}:/app`,
    '-v', 'coder-cargo-registry:/usr/local/cargo/registry',
    '-v', `${NEXTEST_BIN_VOLUME}:/usr/local/cargo/bin`,
    '-v', 'coder-cargo-git:/usr/local/cargo/git',
    '-w', '/app',
    IMAGE,
    'cargo', 'install', 'cargo-nextest', '--locked', '--root', '/usr/local/cargo',
  ], { stdio: 'inherit' });
  if (install.status !== 0) throw new Error('install cargo-nextest failed');
}

// 启动一个共享 MySQL 容器（host network + 固定端口），返回 { port }。
// 已存在且 ping 健康则直接复用，跳过 ~10s 容器冷启。
// 不健康才 rm + 重建；mysqladmin ping 通过才算就绪（≤60s）。
function startSharedMysql() {
  const inspect = spawnSync('docker', ['inspect', '--type=container', MYSQL_CONTAINER_NAME], { encoding: 'utf8' });
  if (inspect.status === 0) {
    const ping = spawnSync('docker', [
      'exec', MYSQL_CONTAINER_NAME, 'mysqladmin', 'ping', '-uroot', `--port=${MYSQL_PORT}`,
    ], { encoding: 'utf8' });
    if (ping.status === 0 && /mysqld is alive/.test(ping.stdout || '')) {
      console.log(`[mysql] reusing ${MYSQL_CONTAINER_NAME} on host port ${MYSQL_PORT}`);
      return { port: MYSQL_PORT };
    }
  }
  spawnSync('docker', ['rm', '-f', MYSQL_CONTAINER_NAME], { stdio: 'ignore' });
  console.log(`[mysql] starting shared ${MYSQL_IMAGE} on host port ${MYSQL_PORT}...`);
  const run = spawnSync('docker', [
    'run', '-d',
    '--name', MYSQL_CONTAINER_NAME,
    '--network', 'host',
    '-e', 'MYSQL_ROOT_PASSWORD=',
    '-e', 'MYSQL_ALLOW_EMPTY_PASSWORD=yes',
    MYSQL_IMAGE,
    '--port=' + MYSQL_PORT,
    '--innodb-use-native-aio=0',
    '--max-connections=500',
    '--skip-log-bin',
  ], { encoding: 'utf8' });
  if (run.status !== 0) {
    console.error('[mysql] docker run failed:', run.stderr);
    throw new Error('start shared mysql failed');
  }
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    const ping = spawnSync('docker', [
      'exec', MYSQL_CONTAINER_NAME,
      'mysqladmin', 'ping', '-uroot', `--port=${MYSQL_PORT}`,
    ], { encoding: 'utf8' });
    if (ping.status === 0 && /mysqld is alive/.test(ping.stdout || '')) {
      console.log(`[mysql] ready on 127.0.0.1:${MYSQL_PORT}`);
      return { port: MYSQL_PORT };
    }
    spawnSync('sleep', ['1']);
  }
  throw new Error(`shared mysql not ready in 60s`);
}

function stopSharedMysql() {
  // ponytail: 保留容器，下一次 run 走 reuse 路径省 ~10s 冷启。
  // ponytail: 想要强制重建就手动 `docker rm -f coder-test-mysql`。
}

// 列出当前所有 mysql:* / testcontainers ryuk 的容器 id（运行中 + 已退出）。
// 用于在测试前后做差分，仅清理本次新生出来的 testcontainers 容器，避免误伤宿主上其它 mysql。
function listTestContainerIds() {
  const ids = new Set();
  const filters = [
    ['--filter', 'ancestor=mysql:8.0'],
    ['--filter', 'ancestor=mysql:8.1'],
    ['--filter', 'ancestor=mysql:8'],
    ['--filter', 'ancestor=mysql:latest'],
    ['--filter', 'name=testcontainers'],
    ['--filter', 'label=org.testcontainers'],
  ];
  for (const f of filters) {
    const res = spawnSync('docker', ['ps', '-aq', ...f], { encoding: 'utf8' });
    if (res.status === 0 && res.stdout) {
      for (const id of res.stdout.trim().split('\n').filter(Boolean)) ids.add(id);
    }
  }
  return ids;
}

function cleanupNewContainers(beforeIds) {
  const after = listTestContainerIds();
  const newOnes = [...after].filter(id => !beforeIds.has(id));
  if (newOnes.length === 0) return;
  console.log(`\n[cleanup] removing ${newOnes.length} test mysql container(s)...`);
  spawnSync('docker', ['rm', '-f', ...newOnes], { stdio: 'inherit' });
}

// 阶段 A：只编译，不跑。`--no-run` 让 cargo 跳过 test runner 配置和 fork。
function runCompile(svc, sharedEnv) {
  console.log(`\n[compile] ${svc}`);
  return dockerRun(svc, sharedEnv,
    'cargo', 'nextest', 'run', '--no-run', '--target', TARGET, '-p', svc,
  );
}

// 阶段 B：跑测试（编译产物已就绪，cargo 检查 fingerprint 后直接 fork）。
// nextest 默认按 CPU 数并行 test 进程；DB 串行问题由 nextest 内部调度缓解（每个 test 独立进程）。
function runRun(svc, sharedEnv) {
  console.log(`\n[run] ${svc}`);
  return dockerRun(svc, sharedEnv,
    'cargo', 'nextest', 'run', '--target', TARGET, '-p', svc,
  );
}

function dockerRun(svc, sharedEnv, ...cmd) {
  const args = [
    'run', '--rm',
    '--network', 'host',
    '-v', `${ROOT}:/app`,
    '-v', 'coder-cargo-registry:/usr/local/cargo/registry',
    '-v', `${NEXTEST_BIN_VOLUME}:/usr/local/cargo/bin`,
    '-v', 'coder-cargo-git:/usr/local/cargo/git',
    '-v', 'coder-cargo-target:/app/target',
    '-v', '/var/run/docker.sock:/var/run/docker.sock',
  ];
  if (sharedEnv) {
    args.push('-e', `TEST_MYSQL_HOST=127.0.0.1`);
    args.push('-e', `TEST_MYSQL_PORT=${sharedEnv.port}`);
  }
  args.push('-w', '/app', IMAGE, ...cmd);
  return new Promise((resolve) => {
    const child = spawn('docker', args, { stdio: 'inherit', cwd: ROOT });
    child.on('error', (err) => {
      console.error(`[${svc}] spawn error:`, err);
      resolve(false);
    });
    child.on('exit', (code) => resolve(code === 0));
  });
}

// 池化调度：固定 concurrency 个槽，任一完成立即补下一个，直到全部跑完。
// runner: (svc, sharedEnv) => Promise<bool>。
async function runPool(targets, concurrency, runner, sharedEnv) {
  const failed = [];
  let cursor = 0;
  const workers = Array.from({ length: Math.min(concurrency, targets.length) }, async () => {
    while (true) {
      const idx = cursor++;
      if (idx >= targets.length) return;
      const svc = targets[idx];
      const ok = await runner(svc, sharedEnv);
      if (!ok) failed.push(svc);
    }
  });
  await Promise.all(workers);
  return failed;
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const incremental = rawArgs.includes('--incremental');
  const userTargets = pickTargets(rawArgs);
  let targets = userTargets;
  if (incremental) {
    const changed = pickIncrementalTargets(rawArgs);
    if (changed === null) {
      console.error('[incremental] git status failed; running user-specified services');
    } else if (changed.length === 0) {
      // pickIncrementalTargets 已经打印 nothing to test
      targets = [];
    } else {
      targets = userTargets.filter(s => changed.includes(s));
      if (targets.length === 0) {
        console.log('[incremental] none of the user-specified services have recent changes');
      } else if (targets.length !== userTargets.length) {
        console.log('[incremental] narrowed to:', targets.join(', '));
      }
    }
  }
  if (targets.length === 0) {
    console.log('\n=== summary ===');
    console.log('total : 0');
    console.log('pass  : 0');
    console.log('fail  : 0');
    process.exit(0);
  }
  const sharedMode = pickSharedMysql(rawArgs);

  const before = sharedMode ? null : listTestContainerIds();
  let sharedEnv = null;

  // 包装一次清理，确保 Ctrl-C / 异常退出也能跑到
  let cleaned = false;
  const doCleanup = () => {
    if (cleaned) return;
    cleaned = true;
    if (sharedMode) {
      try { stopSharedMysql(); } catch (e) { console.error('[cleanup] shared mysql stop failed:', e); }
    } else {
      try { cleanupNewContainers(before); } catch (e) { console.error('[cleanup] failed:', e); }
    }
  };
  process.on('SIGINT',  () => { doCleanup(); process.exit(130); });
  process.on('SIGTERM', () => { doCleanup(); process.exit(143); });
  process.on('exit', doCleanup);

  ensureNextest();
  if (sharedMode) {
    sharedEnv = startSharedMysql();
  }

  const failed = [];
  try {
    // 阶段 A：编译。失败的 service 跳过阶段 B。
    console.log(`\n=== phase 1: compile (concurrency=${CONCURRENCY_COMPILE}) ===`);
    const failedCompile = new Set(await runPool(targets, CONCURRENCY_COMPILE, runCompile, sharedEnv));
    for (const s of failedCompile) failed.push(`${s} (compile)`);

    // 阶段 B：跑测试。仅跑编译成功的。
    const runnable = targets.filter(s => !failedCompile.has(s));
    if (runnable.length > 0) {
      console.log(`\n=== phase 2: run (concurrency=${CONCURRENCY_RUN}) ===`);
      const failedRun = await runPool(runnable, CONCURRENCY_RUN, runRun, sharedEnv);
      for (const s of failedRun) failed.push(`${s} (run)`);
    }
  } finally {
    doCleanup();
  }
  console.log('\n=== summary ===');
  console.log(`total : ${targets.length}`);
  console.log(`pass  : ${targets.length - failed.length}`);
  console.log(`fail  : ${failed.length}`);
  for (const f of failed) console.log(`  - ${f}`);
  process.exit(failed.length ? 1 : 0);
}

main();