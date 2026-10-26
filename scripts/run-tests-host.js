#!/usr/bin/env node
// 在主机上跑各 service 的 cargo 测试（不进 Docker 编译容器）。
//
// 与 scripts/run-tests-docker.js 的分工：
//   - run-tests-docker.js：编译 + 测试都在 Linux/musl 容器里，产物与发布镜像一致，
//     用于 CI 与发布前回归。
//   - run-tests-host.js：编译 + 测试在主机上跑，Windows 默认走 WSL2 发行版
//     （原生 Linux 工具链，Rust 版本与构建镜像一致），迭代快、增量编译复用宿主
//     target 目录，不受容器内 musl target 限制。
//
// 数据库：仍复用 Docker 里的共享 MySQL/Redis 容器。WSL 通过镜像网络能直连
// Windows 侧的 127.0.0.1:33306（MySQL）/ 36379（Redis），不需要在 WSL 里再装一套。
//
// 用法：
//   node scripts/run-tests-host.js                     # 全部 service
//   node scripts/run-tests-host.js iam-service         # 指定 service（可多个）
//   node scripts/run-tests-host.js --incremental       # 只跑 git 改动涉及的 service
//   node scripts/run-tests-host.js --no-db             # 纯单测，不注入数据库 env
//   node scripts/run-tests-host.js --backend native    # 强制本机 cargo（Linux/macOS）
//   node scripts/run-tests-host.js --distro debian-rust
//   node scripts/run-tests-host.js --mysql-host 127.0.0.1 --mysql-port 33306
//   node scripts/run-tests-host.js --redis-url redis://127.0.0.1:36379
const fs = require('node:fs');
const path = require('node:path');
const { chalk, section } = require('./lib/ui.js');
const { resolveRunner, hasNextest, toWslPath, fileUrl } = require('./lib/host-runner.js');
const { pickTargets, pickIncrementalTargets, SERVICE_PKG } = require('./lib/services.js');
const { ensureMysql, ensureRedis } = require('./lib/shared-db.js');

const ROOT = path.join(__dirname, '..');

// utoipa-swagger-ui build.rs 需要的 vendor zip；缺失时回退到直连 github.com（常被 SSL 拦截）。
// 维护命令：`pnpm run assets:download`。
const SWAGGER_UI_ZIP = path.join(ROOT, 'infrastructure', 'vendor', 'swagger-ui-v5.17.14.zip');
const SWAGGER_UI_ENV_KEY = 'SWAGGER_UI_DOWNLOAD_URL';

// 共享测试库默认地址：Docker 里的 coder-test-mysql / coder-test-redis（host network）。
const DEFAULT_MYSQL_HOST = '127.0.0.1';
const DEFAULT_MYSQL_PORT = 33306;
const DEFAULT_REDIS_URL = 'redis://127.0.0.1:36379';
const DB_NAME = 'superpowers';

function parseArgs(argv) {
  const opts = {
    help: false,
    backend: null,
    distro: process.env.WSL_DISTRO || null,
    incremental: false,
    useDb: true,
    mysqlHost: process.env.TEST_MYSQL_HOST || DEFAULT_MYSQL_HOST,
    mysqlPort: Number(process.env.TEST_MYSQL_PORT || DEFAULT_MYSQL_PORT),
    redisUrl: process.env.TEST_REDIS_URL || DEFAULT_REDIS_URL,
    services: [],
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '--help': case '-h': opts.help = true; break;
      case '--backend': opts.backend = argv[++i] || null; break;
      case '--distro': opts.distro = argv[++i] || null; break;
      case '--incremental': opts.incremental = true; break;
      case '--no-db': opts.useDb = false; break;
      case '--mysql-host': opts.mysqlHost = argv[++i] || opts.mysqlHost; break;
      case '--mysql-port': opts.mysqlPort = Number(argv[++i]) || opts.mysqlPort; break;
      case '--redis-url': opts.redisUrl = argv[++i] || opts.redisUrl; break;
      default:
        if (a.startsWith('-')) {
          console.warn(chalk.yellow(`[warn] 忽略未知参数: ${a}`));
        } else {
          opts.services.push(a);
        }
    }
  }
  return opts;
}

// vendor zip 的 file:// URL（按后端转换路径）。
function swaggerEnv(runner) {
  if (process.env[SWAGGER_UI_ENV_KEY]) {
    return { [SWAGGER_UI_ENV_KEY]: process.env[SWAGGER_UI_ENV_KEY] };
  }
  if (!fs.existsSync(SWAGGER_UI_ZIP)) {
    console.warn(chalk.yellow(`[swagger-vendor] zip 不存在: ${SWAGGER_UI_ZIP}`));
    console.warn(chalk.yellow('[swagger-vendor] 先跑 `pnpm run assets:download`，否则 build.rs 会去连 github.com'));
    return {};
  }
  const abs = runner.backend === 'wsl' ? toWslPath(SWAGGER_UI_ZIP) : SWAGGER_UI_ZIP;
  return { [SWAGGER_UI_ENV_KEY]: fileUrl(abs) };
}

// 解析 redis://host:port/db 形式的 URL。
function parseRedisUrl(url) {
  const m = /^redis:\/\/(?:[^@]*@)?([^:/]+)(?::(\d+))?/.exec(url);
  if (!m) return { host: '127.0.0.1', port: 6379 };
  return { host: m[1], port: m[2] ? Number(m[2]) : 6379 };
}

function buildEnv(runner, opts) {  const env = { ...swaggerEnv(runner) };
  if (!opts.useDb) return env;
  env.TEST_MYSQL_HOST = opts.mysqlHost;
  env.TEST_MYSQL_PORT = String(opts.mysqlPort);
  env.DATABASE_URL = `mysql://root:@${opts.mysqlHost}:${opts.mysqlPort}/${DB_NAME}`;
  env.REDIS_URL = opts.redisUrl;
  env.REDIS_PREFIX = `${DB_NAME}:`;
  env.CONSUL_ADDR = 'http://localhost:8500';
  return env;
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const opts = parseArgs(rawArgs);

  if (opts.help) {
    console.log(`usage: node scripts/run-tests-host.js [services...] [options]

在主机上跑各 service 的 cargo 测试（不进 Docker 编译容器）。

选项：
  --backend <wsl|native>  执行后端（Windows 默认 wsl，其它平台 native）
  --distro <name>         指定 WSL 发行版（默认挑一个装了 cargo 的）
  --incremental           只跑 git 改动涉及的 service
  --no-db                 纯单测，不注入数据库 env
  --mysql-host <host>     MySQL 地址（默认 ${DEFAULT_MYSQL_HOST}）
  --mysql-port <port>     MySQL 端口（默认 ${DEFAULT_MYSQL_PORT}）
  --redis-url <url>       Redis 地址（默认 ${DEFAULT_REDIS_URL}）
  -h, --help              显示帮助

service 列表：${Object.keys(SERVICE_PKG).join(', ')}
`);
    return;
  }

  let targets = pickTargets(rawArgs);
  if (opts.incremental) {
    const changed = pickIncrementalTargets(rawArgs);
    if (changed === null) {
      console.error('[incremental] git status 失败，回退到全量 service');
    } else {
      targets = targets.filter((t) => changed.includes(t));
      if (targets.length === 0) {
        section('summary');
        console.log('total : 0');
        console.log('pass  : 0');
        console.log('fail  : 0');
        process.exit(0);
      }
    }
  }

  const runner = resolveRunner({ backend: opts.backend, distro: opts.distro });
  console.log(chalk.bold('\n=== host test runner ==='));
  console.log(`  backend : ${runner.backend}${runner.distro ? ` (${runner.distro})` : ''}`);
  console.log(`  repo    : ${runner.repoPath}`);
  console.log(`  services: ${targets.join(', ')} (${targets.length})`);

  if (opts.useDb) {
    const { host: redisHost, port: redisPort } = parseRedisUrl(opts.redisUrl);
    const mysqlOk = await ensureMysql(opts.mysqlHost, opts.mysqlPort);
    const redisOk = await ensureRedis(redisHost, redisPort);
    if (!mysqlOk) console.warn(chalk.yellow('[db] MySQL 不可达，需要数据库的测试会失败'));
    if (!redisOk) console.warn(chalk.yellow('[db] Redis 不可达，需要 Redis 的测试会失败'));
  }

  const env = buildEnv(runner, opts);
  const useNextest = hasNextest(runner);
  console.log(`  runner  : cargo ${useNextest ? 'nextest' : 'test'}`);

  const failed = [];

  // 阶段 A：只编译。失败的 service 跳过阶段 B。
  section('phase 1: compile');
  const compileFailed = new Set();
  for (const svc of targets) {
    console.log(chalk.bold(`\n[compile] ${svc}`));
    const args = useNextest
      ? ['nextest', 'run', '--no-run', '-p', SERVICE_PKG[svc]]
      : ['test', '--no-run', '-p', SERVICE_PKG[svc]];
    const res = runner.run('cargo', args, { env });
    if (res.status !== 0) {
      compileFailed.add(svc);
      failed.push(`${svc} (compile)`);
    }
  }

  // 阶段 B：跑测试。只跑编译成功的。
  const runnable = targets.filter((s) => !compileFailed.has(s));
  if (runnable.length > 0) {
    section('phase 2: run');
    for (const svc of runnable) {
      console.log(chalk.bold(`\n[run] ${svc}`));
      const args = useNextest
        ? ['nextest', 'run', '-p', SERVICE_PKG[svc]]
        : ['test', '-p', SERVICE_PKG[svc]];
      const res = runner.run('cargo', args, { env });
      if (res.status !== 0) failed.push(`${svc} (run)`);
    }
  }

  section('summary');
  console.log(`total : ${targets.length}`);
  console.log(`pass  : ${chalk.green(targets.length - failed.length)}`);
  console.log(`fail  : ${failed.length ? chalk.red(failed.length) : failed.length}`);
  for (const f of failed) console.log(`  ${chalk.red('-')} ${f}`);
  process.exit(failed.length ? 1 : 0);
}

main().catch((err) => {
  console.error(`run-tests-host failed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
